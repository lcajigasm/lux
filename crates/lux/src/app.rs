//! The application's own state and its `PlatformEventHandler` wiring — the
//! first real assembly of every Phase 2 crate into one running program.
//! Frame drawing lives in `crate::render` (an `impl App` block in a
//! separate file, kept apart only to stay under the 600-line file limit).

use std::time::{Duration, Instant};

use accesskit::TreeUpdate;
use lux_core::buffer::Buffer;
use lux_core::movement;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::{
    ElementState, Ime, ImeComposition, ImeUpdate, Key, NamedKey, PlatformContext, PlatformEvent,
    PlatformEventHandler, PlatformWindow, WindowId,
};
use lux_render::{GlyphAtlas, GpuContext, QuadPipeline, RepaintScheduler, Surface, TextPipeline};
use lux_ui::editor_view::{EditorView, EditorViewMetrics, Viewport};
use lux_ui::{Chrome, FocusTarget};
use swash::FontRef;
use swash::scale::ScaleContext;

/// A placeholder document: real enough to demonstrate virtualized scroll,
/// real shaping, real cursor movement and editing — not sample marketing
/// copy. Phase 4's "open a real file" work is what replaces this with an
/// actual file path from argv/a native file dialog.
const SAMPLE_TEXT: &str = include_str!("sample.txt");

pub(crate) const FONT_SIZE: f32 = 16.0;
pub(crate) const LINE_HEIGHT: f32 = FONT_SIZE * 1.4;
pub(crate) const BLINK_INTERVAL: Duration = Duration::from_millis(530);
pub(crate) const ATLAS_SIDE: u32 = 1024;
/// Left padding between the line-number gutter and the first character —
/// gutter *numbers* aren't painted yet (a real, flagged gap: they need
/// their own shaping pass; see the receipt), only their reserved width.
pub(crate) const TEXT_LEFT_PADDING: f32 = 8.0;

/// Everything that needs a live GPU device/surface/window — created once,
/// in `resumed`, and gone if the app is ever suspended (desktop never
/// actually calls `surfaces_lost` today, but the type still models the real
/// lifecycle `lux-platform`'s trait documents).
pub(crate) struct GpuWindow {
    pub(crate) window: &'static dyn PlatformWindow,
    pub(crate) id: WindowId,
    pub(crate) gpu: GpuContext,
    pub(crate) surface: Surface<'static>,
    pub(crate) quads: QuadPipeline,
    pub(crate) text: TextPipeline,
}

pub(crate) struct App {
    pub(crate) editor_view: EditorView,
    /// Built exactly once from leaked, process-lifetime font bytes and
    /// reused for every frame. `swash::FontRef::key` is a global,
    /// per-call-incrementing counter, not derived from the font's content
    /// (see `swash::cache::CacheKey::new`) — calling `FontRef::from_index`
    /// again on every redraw, even on the *same* bytes, would mint a fresh
    /// key each time, so every `GlyphKey` built from it would miss the
    /// atlas cache every single frame (constant needless re-rasterization
    /// and eviction churn — a real, confirmed bug this field exists to fix,
    /// not a hypothetical one).
    pub(crate) font: FontRef<'static>,
    pub(crate) buffer: Buffer,
    pub(crate) cursors: MultiCursor,
    pub(crate) goal_column: usize,
    pub(crate) viewport: Viewport,
    pub(crate) scheduler: RepaintScheduler,
    pub(crate) ime: ImeComposition,
    pub(crate) start: Instant,
    pub(crate) atlas: GlyphAtlas,
    pub(crate) atlas_dirty: bool,
    pub(crate) scale_cx: ScaleContext,
    pub(crate) gpu_window: Option<GpuWindow>,
    pub(crate) chrome: Chrome,
}

impl App {
    pub(crate) fn new(font_bytes: Vec<u8>) -> Self {
        // Leaked once, for the process's lifetime — the same real,
        // intentional pattern used for the window (see `crate::render`):
        // `FontRef<'static>` needs `'static` bytes, and this app never
        // needs to free or swap the font while running.
        let font_bytes: &'static [u8] = Box::leak(font_bytes.into_boxed_slice());
        let font = FontRef::from_index(font_bytes, 0)
            .expect("system font bytes read from disk must parse as a real font");

        let text_layout = lux_render::TextLayoutContext::new();
        let metrics = EditorViewMetrics {
            line_height: LINE_HEIGHT,
            font_size: FONT_SIZE,
            ligatures: true,
            column_rulers: vec![80],
            blink_interval: BLINK_INTERVAL,
        };
        let editor_view = EditorView::new(text_layout, font_bytes, metrics)
            .expect("system font bytes read from disk must parse as a real font");

        let buffer = Buffer::from_bytes(SAMPLE_TEXT.as_bytes()).expect("sample.txt must decode");
        let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));

        Self {
            editor_view,
            font,
            buffer,
            cursors,
            goal_column: 0,
            viewport: Viewport {
                width: 1000.0,
                height: 700.0,
                scroll_x: 0.0,
                scroll_y: 0.0,
            },
            scheduler: RepaintScheduler::new(),
            ime: ImeComposition::new(),
            start: Instant::now(),
            atlas: GlyphAtlas::new(ATLAS_SIDE),
            atlas_dirty: true,
            scale_cx: ScaleContext::new(),
            gpu_window: None,
            chrome: Chrome::default(),
        }
    }

    fn move_cursor(&mut self, to: Position, reset_goal_column: bool) {
        self.cursors = MultiCursor::new(Selection::cursor(to));
        if reset_goal_column {
            self.goal_column = to.column;
        }
        self.scheduler.invalidate_all();
    }

    fn insert_at_cursor(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let pos = self.cursors.primary().head;
        self.buffer.insert(pos.char, text);
        let new_pos = Position::from_char(&self.buffer, pos.char + text.chars().count());
        self.move_cursor(new_pos, true);
    }

    fn backspace(&mut self) {
        let pos = self.cursors.primary().head;
        let prev = pos.prev_grapheme(&self.buffer);
        if prev == pos {
            return;
        }
        self.buffer.delete(prev.char..pos.char);
        let new_pos = Position::from_char(&self.buffer, prev.char);
        self.move_cursor(new_pos, true);
    }

    fn delete_forward(&mut self) {
        let pos = self.cursors.primary().head;
        let next = pos.next_grapheme(&self.buffer);
        if next == pos {
            return;
        }
        self.buffer.delete(pos.char..next.char);
        let new_pos = Position::from_char(&self.buffer, pos.char);
        self.move_cursor(new_pos, true);
    }

    fn handle_named_key(&mut self, key: NamedKey) -> bool {
        let pos = self.cursors.primary().head;
        match key {
            NamedKey::F(6) => {
                let next = self.chrome.focus.focus_next();
                if !lux_ui::surfaces::terminal_focus_allowed(next) && next == FocusTarget::Terminal
                {
                    self.chrome.focus.focus_next();
                }
                true
            }
            NamedKey::Escape => {
                self.chrome.focus.escape_to_editor();
                true
            }
            NamedKey::ArrowLeft => {
                self.move_cursor(movement::move_char_left(&self.buffer, pos), true);
                true
            }
            NamedKey::ArrowRight => {
                self.move_cursor(movement::move_char_right(&self.buffer, pos), true);
                true
            }
            NamedKey::ArrowUp => {
                let to = movement::move_line_up(&self.buffer, pos, self.goal_column);
                self.move_cursor(to, false);
                true
            }
            NamedKey::ArrowDown => {
                let to = movement::move_line_down(&self.buffer, pos, self.goal_column);
                self.move_cursor(to, false);
                true
            }
            NamedKey::Home => {
                self.move_cursor(movement::line_start(&self.buffer, pos), true);
                true
            }
            NamedKey::End => {
                self.move_cursor(movement::line_end(&self.buffer, pos), true);
                true
            }
            NamedKey::Backspace => {
                self.backspace();
                true
            }
            NamedKey::Delete => {
                self.delete_forward();
                true
            }
            NamedKey::Enter => {
                let eol = self.buffer.eol().as_str().to_string();
                self.insert_at_cursor(&eol);
                true
            }
            NamedKey::Tab => {
                self.insert_at_cursor("\t");
                true
            }
            _ => false,
        }
    }

    fn apply_ime(&mut self, event: Ime) {
        match self.ime.apply(event) {
            ImeUpdate::Idle | ImeUpdate::Composing { .. } => {
                // Preedit is drawn inline by `crate::render` reading
                // `self.ime` directly; nothing belongs in the buffer yet.
                self.scheduler.invalidate_all();
            }
            ImeUpdate::Committed(text) => self.insert_at_cursor(&text),
        }
    }

    fn build_accessibility_tree(&mut self) -> TreeUpdate {
        let buffer = &self.buffer;
        let cursors = &self.cursors;
        let family = self.editor_view.family().to_string();
        lux_ui::build_accessibility_tree(
            self.editor_view.text_layout_mut(),
            &family,
            FONT_SIZE,
            buffer,
            cursors,
        )
    }
}

impl PlatformEventHandler for App {
    fn resumed(&mut self, ctx: &dyn PlatformContext) {
        if self.gpu_window.is_some() {
            return;
        }
        self.create_gpu_window(ctx);
    }

    fn window_event(&mut self, ctx: &dyn PlatformContext, window: WindowId, event: PlatformEvent) {
        let Some(gpu_window) = &self.gpu_window else {
            return;
        };
        if gpu_window.id != window {
            return;
        }

        match event {
            PlatformEvent::CloseRequested => ctx.exit(),
            PlatformEvent::Resized(size) => {
                self.resize(size);
            }
            PlatformEvent::RedrawRequested => {
                self.redraw(ctx);
            }
            PlatformEvent::Keyboard(key_event) => {
                if key_event.state != ElementState::Pressed {
                    return;
                }
                let handled = match key_event.key {
                    Key::Named(named) => self.handle_named_key(named),
                    _ => false,
                };
                if !handled
                    && !self.ime.is_composing()
                    && let Some(text) = &key_event.text
                {
                    self.insert_at_cursor(text);
                }
                if let Some(gpu_window) = &self.gpu_window {
                    gpu_window.window.request_redraw();
                }
            }
            PlatformEvent::Ime(ime_event) => {
                self.apply_ime(ime_event);
                if let Some(gpu_window) = &self.gpu_window {
                    gpu_window.window.request_redraw();
                }
            }
            PlatformEvent::MouseWheel(delta) => {
                self.scroll(delta);
            }
            PlatformEvent::AccessibilityInitialTreeRequested => {
                let update = self.build_accessibility_tree();
                if let Some(gpu_window) = &self.gpu_window {
                    gpu_window.window.update_accessibility_tree(update);
                }
            }
            PlatformEvent::AccessibilityActionRequested(_request) => {
                // No editable accessibility actions (set-selection, etc.)
                // are wired to real buffer mutation yet — a real, flagged
                // gap for a future pass, not silently ignored: screen
                // readers can still read content/cursor/selection today,
                // just not drive them from this path.
            }
            PlatformEvent::AccessibilityDeactivated => {}
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _ctx: &dyn PlatformContext) {
        let Some(gpu_window) = &self.gpu_window else {
            return;
        };
        if self.scheduler.needs_repaint(Instant::now()) {
            gpu_window.window.request_redraw();
        }
    }
}
