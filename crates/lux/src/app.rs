//! The application's own state and its `PlatformEventHandler` wiring — the
//! first real assembly of every Phase 2 crate into one running program.
//! Frame drawing lives in `crate::render` (an `impl App` block in a
//! separate file, kept apart only to stay under the 600-line file limit).

use std::time::{Duration, Instant};

use accesskit::TreeUpdate;
use lux_core::buffer::Buffer;
use lux_core::history::History;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::{
    Ime, ImeComposition, ImeUpdate, Modifiers, PlatformContext, PlatformEvent,
    PlatformEventHandler, PlatformWindow, WindowId,
};
use lux_render::{GlyphAtlas, GpuContext, QuadPipeline, RepaintScheduler, Surface, TextPipeline};
use lux_ui::Chrome;
use lux_ui::editor_view::{EditorView, EditorViewMetrics, Viewport};
use swash::FontRef;
use swash::scale::ScaleContext;

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
    pub(crate) scale_factor: f32,
    pub(crate) presentation: crate::presentation::Presentation,
    pub(crate) documents: crate::document::Documents,
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
    pub(crate) goal_columns: Vec<usize>,
    pub(crate) history: History,
    pub(crate) modifiers: Modifiers,
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

        let buffer = Buffer::from_bytes(b"").expect("empty document decodes");
        let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));

        Self {
            scale_factor: 1.0,
            presentation: Default::default(),
            documents: crate::document::Documents::default(),
            editor_view,
            font,
            buffer,
            cursors,
            goal_columns: vec![0],
            history: History::new(),
            modifiers: Modifiers::default(),
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

    pub(crate) fn apply_ime(&mut self, event: Ime) {
        if self.editing_blocked() {
            return;
        }
        match self.ime.apply(event) {
            ImeUpdate::Idle | ImeUpdate::Composing { .. } => {
                // Preedit is drawn inline by `crate::render` reading
                // `self.ime` directly; nothing belongs in the buffer yet.
                self.scheduler.invalidate_all();
            }
            ImeUpdate::Committed(text) => self.insert_at_cursor(&text),
        }
        self.update_document_title();
    }

    fn build_accessibility_tree(&mut self) -> TreeUpdate {
        let buffer = &self.buffer;
        let cursors = &self.cursors;
        let family = self.editor_view.family().to_string();
        lux_ui::build_accessibility_tree(
            self.editor_view.text_layout_mut(),
            &family,
            FONT_SIZE * self.scale_factor,
            buffer,
            cursors,
        )
    }
}

impl PlatformEventHandler for App {
    fn background_event(&mut self, ctx: &dyn PlatformContext) {
        self.finish_document_operation(ctx);
    }
    fn resumed(&mut self, ctx: &dyn PlatformContext) {
        if self.gpu_window.is_some() {
            return;
        }
        self.create_gpu_window(ctx);
        self.update_document_title();
        if let Some(path) = self.documents.initial_path.take() {
            self.request_document(ctx, crate::document::Request::Open(Some(path)));
        }
    }

    fn window_event(&mut self, ctx: &dyn PlatformContext, window: WindowId, event: PlatformEvent) {
        let Some(gpu_window) = &self.gpu_window else {
            return;
        };
        if gpu_window.id != window {
            return;
        }

        match event {
            PlatformEvent::CloseRequested => {
                self.request_document(ctx, crate::document::Request::Close)
            }
            PlatformEvent::Resized(size) => {
                self.resize(size);
            }
            PlatformEvent::ScaleFactorChanged { scale_factor } => {
                self.set_display_scale(scale_factor);
                if let Some(gpu) = &self.gpu_window {
                    gpu.window.request_redraw();
                }
            }
            PlatformEvent::Occluded(hidden) => {
                self.presentation.set_hidden(hidden);
                if !hidden {
                    self.scheduler.invalidate_all();
                    if let Some(gpu) = &self.gpu_window {
                        gpu.window.request_redraw();
                    }
                }
            }
            PlatformEvent::Focused(true) => {
                self.presentation.set_hidden(false);
                self.scheduler.invalidate_all();
                if let Some(gpu) = &self.gpu_window {
                    gpu.window.request_redraw();
                }
            }
            PlatformEvent::RedrawRequested => {
                self.redraw(ctx);
            }
            PlatformEvent::Keyboard(key_event) => {
                if !self.document_key(ctx, &key_event) {
                    self.handle_key(&key_event);
                }
                self.update_document_title();
                if let Some(gpu_window) = &self.gpu_window {
                    gpu_window.window.request_redraw();
                }
            }
            PlatformEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers,
            PlatformEvent::Focused(false) => self.modifiers = Modifiers::default(),
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
        if self.presentation.ready(Instant::now()) && self.scheduler.needs_repaint(Instant::now()) {
            gpu_window.window.request_redraw();
        }
    }
}
