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
use std::path::PathBuf;

use lux_platform::{
    ElementState, FileDialogOptions, Ime, ImeComposition, ImeUpdate, Key, Modifiers, NamedKey,
    PlatformContext, PlatformEvent, PlatformEventHandler, PlatformWindow, WindowId,
};
use lux_render::{GlyphAtlas, GpuContext, QuadPipeline, RepaintScheduler, Surface, TextPipeline};
use lux_ui::editor_view::{EditorView, EditorViewMetrics, Viewport};
use lux_ui::{Chrome, FocusTarget};
use swash::FontRef;
use swash::scale::ScaleContext;

/// Fallback document when the binary is launched with no path argument and
/// the user has not opened a file yet. Real open/save (G02) replaces this
/// with an on-disk path; the sample remains a legitimate untitled buffer,
/// not a stub API.
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
    /// On-disk path for the current buffer, if any. `None` means untitled.
    pub(crate) path: Option<PathBuf>,
    pub(crate) dirty: bool,
    /// Huge files open read-only (see `document::open_path`).
    pub(crate) read_only: bool,
    pub(crate) modifiers: Modifiers,
    /// Layered settings + keymap dispatch (G03). `None` only when
    /// bootstrap failed; the shell then keeps hardcoded fallbacks.
    pub(crate) config: Option<crate::config::RuntimeConfig>,
}

impl App {
    pub(crate) fn new(font_bytes: Vec<u8>, open_path: Option<PathBuf>) -> Self {
        // Leaked once, for the process's lifetime — the same real,
        // intentional pattern used for the window (see `crate::render`):
        // `FontRef<'static>` needs `'static` bytes, and this app never
        // needs to free or swap the font while running.
        let font_bytes: &'static [u8] = Box::leak(font_bytes.into_boxed_slice());
        let font = FontRef::from_index(font_bytes, 0)
            .expect("system font bytes read from disk must parse as a real font");

        let text_layout = lux_render::TextLayoutContext::new();
        let config = crate::config::RuntimeConfig::bootstrap();
        let metrics = match &config {
            Some(cfg) => cfg.editor_metrics(),
            None => EditorViewMetrics {
                line_height: LINE_HEIGHT,
                font_size: FONT_SIZE,
                ligatures: true,
                column_rulers: vec![80],
                blink_interval: BLINK_INTERVAL,
            },
        };
        let editor_view = EditorView::new(text_layout, font_bytes, metrics)
            .expect("system font bytes read from disk must parse as a real font");

        let (buffer, path, read_only) = match open_path {
            Some(path) => match crate::document::open_path(&path) {
                Ok(opened) => {
                    println!("Lux: opened {}", opened.path.display());
                    if opened.read_only {
                        println!(
                            "Lux: {} is huge — opened read-only (no fake preview)",
                            opened.path.display()
                        );
                    }
                    (opened.buffer, Some(opened.path), opened.read_only)
                }
                Err(err) => {
                    eprintln!("Lux: {err}");
                    eprintln!("Lux: falling back to the untitled sample buffer");
                    (
                        Buffer::from_bytes(SAMPLE_TEXT.as_bytes())
                            .expect("sample.txt must decode"),
                        None,
                        false,
                    )
                }
            },
            None => (
                Buffer::from_bytes(SAMPLE_TEXT.as_bytes()).expect("sample.txt must decode"),
                None,
                false,
            ),
        };
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
            path,
            dirty: false,
            read_only,
            modifiers: Modifiers::default(),
            config,
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
        if self.read_only {
            eprintln!("Lux: buffer is read-only (huge file); edit refused");
            return;
        }
        let pos = self.cursors.primary().head;
        self.buffer.insert(pos.char, text);
        let new_pos = Position::from_char(&self.buffer, pos.char + text.chars().count());
        self.move_cursor(new_pos, true);
        self.mark_dirty();
    }

    fn backspace(&mut self) {
        if self.read_only {
            eprintln!("Lux: buffer is read-only (huge file); edit refused");
            return;
        }
        let pos = self.cursors.primary().head;
        let prev = pos.prev_grapheme(&self.buffer);
        if prev == pos {
            return;
        }
        self.buffer.delete(prev.char..pos.char);
        let new_pos = Position::from_char(&self.buffer, prev.char);
        self.move_cursor(new_pos, true);
        self.mark_dirty();
    }

    fn delete_forward(&mut self) {
        if self.read_only {
            eprintln!("Lux: buffer is read-only (huge file); edit refused");
            return;
        }
        let pos = self.cursors.primary().head;
        let next = pos.next_grapheme(&self.buffer);
        if next == pos {
            return;
        }
        self.buffer.delete(pos.char..next.char);
        let new_pos = Position::from_char(&self.buffer, pos.char);
        self.move_cursor(new_pos, true);
        self.mark_dirty();
    }


    fn mark_dirty(&mut self) {
        if !self.dirty {
            self.dirty = true;
            self.refresh_title();
        }
    }

    fn refresh_title(&self) {
        let Some(gpu) = &self.gpu_window else {
            return;
        };
        let title = crate::document::window_title(
            self.path.as_deref(),
            self.dirty,
            self.read_only,
        );
        gpu.window.set_title(&title);
    }

    fn replace_document(
        &mut self,
        buffer: Buffer,
        path: Option<PathBuf>,
        read_only: bool,
        dirty: bool,
    ) {
        self.buffer = buffer;
        self.path = path;
        self.read_only = read_only;
        self.dirty = dirty;
        self.cursors = MultiCursor::new(Selection::cursor(Position::document_start()));
        self.goal_column = 0;
        self.viewport.scroll_x = 0.0;
        self.viewport.scroll_y = 0.0;
        self.scheduler.invalidate_all();
        self.refresh_title();
    }

    fn open_path_on_ui_thread(&mut self, path: PathBuf) {
        match crate::document::open_path(&path) {
            Ok(opened) => {
                println!("Lux: opened {}", opened.path.display());
                if opened.read_only {
                    println!(
                        "Lux: {} is huge — opened read-only (no fake preview)",
                        opened.path.display()
                    );
                }
                self.replace_document(opened.buffer, Some(opened.path), opened.read_only, false);
            }
            Err(err) => eprintln!("Lux: {err}"),
        }
    }

    fn save_to_path_on_ui_thread(&mut self, path: PathBuf) {
        match crate::document::save_buffer(&self.buffer, &path) {
            Ok(()) => {
                println!("Lux: saved {}", path.display());
                self.path = Some(path);
                self.dirty = false;
                self.refresh_title();
            }
            Err(err) => eprintln!("Lux: {err}"),
        }
    }

    fn action_open(&mut self, ctx: &dyn PlatformContext) {
        let options = FileDialogOptions {
            title: Some("Open File".to_string()),
            starting_directory: self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())),
            filters: vec![("Text".to_string(), vec!["txt".to_string(), "md".to_string(), "rs".to_string()])],
        };
        if let Some(path) = ctx.file_dialogs().pick_file(&options) {
            self.open_path_on_ui_thread(path);
        }
    }

    fn action_save(&mut self, ctx: &dyn PlatformContext) {
        if let Some(path) = self.path.clone() {
            self.save_to_path_on_ui_thread(path);
        } else {
            self.action_save_as(ctx);
        }
    }

    fn action_save_as(&mut self, ctx: &dyn PlatformContext) {
        let options = FileDialogOptions {
            title: Some("Save As".to_string()),
            starting_directory: self.path.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf())),
            filters: vec![("Text".to_string(), vec!["txt".to_string(), "md".to_string(), "rs".to_string()])],
        };
        if let Some(path) = ctx.file_dialogs().save_file(&options) {
            self.save_to_path_on_ui_thread(path);
        }
    }

    fn chord_primary(&self) -> bool {
        self.modifiers.control || self.modifiers.meta
    }

    /// Dispatches a single chord through the loaded keymap. Only commands
    /// that this shell actually implements (`save` / `open` / `save_as`) are
    /// indexed; everything else falls through to normal input.
    fn handle_keymap_chord(&mut self, ctx: &dyn PlatformContext, key: &Key) -> bool {
        let Some(config) = self.config.as_ref() else {
            return self.handle_file_chord_fallback(ctx, key);
        };
        let Some(chord) = crate::config::chord_from_key(&self.modifiers, key) else {
            return false;
        };
        let Some(command) = config.lookup_command(&chord).map(str::to_string) else {
            return false;
        };
        match command.as_str() {
            "open" => self.action_open(ctx),
            "save" => self.action_save(ctx),
            "save_as" => self.action_save_as(ctx),
            _ => return false,
        }
        true
    }

    /// Pre-G03 hard chords — used only when config bootstrap failed.
    fn handle_file_chord_fallback(&mut self, ctx: &dyn PlatformContext, key: &Key) -> bool {
        if !self.chord_primary() {
            return false;
        }
        let Key::Character(ch) = key else {
            return false;
        };
        let letter = ch.as_str();
        match (letter, self.modifiers.shift) {
            ("o" | "O", false) => {
                self.action_open(ctx);
                true
            }
            ("s" | "S", false) => {
                self.action_save(ctx);
                true
            }
            ("s" | "S", true) => {
                self.action_save_as(ctx);
                true
            }
            _ => false,
        }
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
        let font_size = self.editor_view.metrics().font_size;
        lux_ui::build_accessibility_tree(
            self.editor_view.text_layout_mut(),
            &family,
            font_size,
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
        self.refresh_title();
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
            PlatformEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
            }
            PlatformEvent::Keyboard(key_event) => {
                if key_event.state != ElementState::Pressed {
                    return;
                }
                if self.handle_keymap_chord(ctx, &key_event.key) {
                    if let Some(gpu_window) = &self.gpu_window {
                        gpu_window.window.request_redraw();
                    }
                    return;
                }
                let handled = match &key_event.key {
                    Key::Named(named) => self.handle_named_key(*named),
                    _ => false,
                };
                if !handled
                    && !self.chord_primary()
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
        if let Some(config) = self.config.as_mut()
            && config.drain_reloads()
        {
            let metrics = config.editor_metrics();
            self.editor_view.set_metrics(metrics);
            self.atlas_dirty = true;
            self.scheduler.invalidate_all();
            if let Some(gpu_window) = &self.gpu_window {
                gpu_window.window.request_redraw();
            }
        }
        let Some(gpu_window) = &self.gpu_window else {
            return;
        };
        if self.scheduler.needs_repaint(Instant::now()) {
            gpu_window.window.request_redraw();
        }
    }
}
