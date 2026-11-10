//! The application's own state and its `PlatformEventHandler` wiring — the
//! first real assembly of every Phase 2 crate into one running program.
//! Frame drawing lives in `crate::render` (an `impl App` block in a
//! separate file, kept apart only to stay under the 600-line file limit).

use std::time::{Duration, Instant};

use accesskit::{ActionData, TreeUpdate};
use lux_core::buffer::Buffer;
use lux_core::history::History;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::event::KeyEvent;
use lux_platform::{
    ElementState, Ime, ImeComposition, ImeUpdate, Key, Modifiers, MouseButton, NamedKey,
    PlatformContext, PlatformEvent, PlatformEventHandler, WindowId,
};
use lux_render::{GlyphAtlas, RepaintScheduler};
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
pub(crate) const TAB_BAR_HEIGHT: f32 = 32.0;
pub(crate) const TAB_MIN_WIDTH: f32 = 120.0;
pub(crate) const TAB_MAX_WIDTH: f32 = 200.0;
pub(crate) const TAB_CLOSE_BTN_WIDTH: f32 = 24.0;

mod bookmarks;
mod folding;
mod selection;
mod syntax;
mod types;
mod views;
pub(crate) use types::{AtlasState, GpuWindow, MouseState, SyntaxState};

pub(crate) struct App {
    pub(crate) scale_factor: f32,
    pub(crate) presentation: crate::presentation::Presentation,
    pub(crate) documents: crate::document::Documents,
    pub(crate) editor_view: EditorView,
    pub(crate) fold_state: lux_core::folding::FoldState,
    /// Built exactly once from leaked, process-lifetime font bytes and
    /// reused for every frame. The swash font key is a global
    /// per-call-incrementing counter, not derived from the font's content
    /// (see swash cache CacheKey) — calling FontRef from index
    /// again on every redraw, even on the same bytes, would mint a fresh
    /// key each time, so every GlyphKey built from it would miss the
    /// atlas cache every single frame (constant needless re-rasterization
    /// and eviction churn — a real, confirmed bug this field exists to fix,
    /// not a hypothetical one).
    pub(crate) font: FontRef<'static>,
    pub(crate) buffer: Buffer,
    pub(crate) cursors: MultiCursor,
    pub(crate) goal_columns: Vec<usize>,
    pub(crate) history: History,
    pub(crate) modifiers: Modifiers,
    pub(crate) mouse: MouseState,
    pub(crate) accessibility_active: bool,
    pub(crate) viewport: Viewport,
    pub(crate) scheduler: RepaintScheduler,
    pub(crate) ime: ImeComposition,
    pub(crate) start: Instant,
    pub(crate) atlas_state: AtlasState,
    pub(crate) gpu_window: Option<GpuWindow>,
    pub(crate) chrome: Chrome,
    pub(crate) keymap: lux_config::Keymap,
    pub(crate) snippet_session: Option<lux_core::snippets::SnippetSession>,
    pub(crate) syntax: Option<SyntaxState>,
    pub(crate) ui: crate::ui::UiOverlays,
    pub(crate) bookmarks: lux_core::bookmarks::BookmarkState,
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
            word_wrap: false,
            column_rulers: Vec::new(),
            blink_interval: BLINK_INTERVAL,
        };
        let mut editor_view = EditorView::new(text_layout, font_bytes, metrics)
            .expect("system font bytes read from disk must parse as a real font");
        crate::font::install_coverage_fallbacks(editor_view.text_layout_mut());
        crate::font::install_ui_font(editor_view.text_layout_mut());

        let buffer = Buffer::from_bytes(b"").expect("empty document decodes");
        let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));

        let mut documents = crate::document::Documents::default();
        if let Some(first) = Self::recoverable_documents().first() {
            documents.message = Some(format!(
                "Unsaved recovery session available: {}",
                first
                    .file_name()
                    .unwrap_or(first.as_os_str())
                    .to_string_lossy()
            ));
        }

        Self {
            scale_factor: 1.0,
            presentation: Default::default(),
            documents,
            editor_view,
            fold_state: lux_core::folding::FoldState::new(),
            bookmarks: lux_core::bookmarks::BookmarkState::new(),
            font,
            buffer,
            cursors,
            goal_columns: vec![0],
            history: History::new(),
            modifiers: Modifiers::default(),
            mouse: MouseState {
                position: dpi::PhysicalPosition::new(0.0, 0.0),
                placed: false,
                selection_anchor: None,
                last_click: None,
            },
            accessibility_active: false,
            viewport: Viewport {
                width: 1000.0,
                height: (700.0 - TAB_BAR_HEIGHT - crate::status_bar::STATUS_BAR_HEIGHT).max(1.0),
                scroll_x: 0.0,
                scroll_y: 0.0,
            },
            scheduler: RepaintScheduler::new(),
            ime: ImeComposition::new(),
            start: Instant::now(),
            atlas_state: AtlasState {
                atlas: GlyphAtlas::new(ATLAS_SIDE),
                dirty: true,
                scale_cx: ScaleContext::new(),
            },
            gpu_window: None,
            chrome: Chrome::default(),
            keymap: lux_config::keymap::presets::sublime(),
            snippet_session: None,
            syntax: None,
            ui: crate::ui::UiOverlays::default(),
        }
    }

    pub(crate) fn apply_ime(&mut self, event: Ime) {
        if crate::field::route_ime(self, &event) {
            return;
        }
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

    pub(crate) fn build_accessibility_tree(&mut self) -> TreeUpdate {
        crate::accessibility::build_accessibility_tree(self)
    }

    pub(crate) fn refresh_accessibility(&mut self) {
        if !self.accessibility_active {
            return;
        }
        let update = self.build_accessibility_tree();
        if let Some(gpu_window) = &self.gpu_window {
            gpu_window.window.update_accessibility_tree(update);
        }
    }

    pub(crate) fn open_language_picker(&mut self) {
        self.ui.language_picker = Some(crate::language_picker::LanguagePickerState::new());
        self.scheduler.invalidate_all();
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.history.state_id() != self.documents.saved_version
    }

    pub(crate) fn is_find_results_active(&self) -> bool {
        crate::find_results::is_find_results_active(self)
    }

    pub(crate) fn find_results_buffer(&self) -> Option<lux_core::buffer::Buffer> {
        crate::find_results::find_results_buffer(self)
    }

    pub(crate) fn handle_mouse_click_on_chrome(
        &mut self,
        ctx: &dyn PlatformContext,
        button: MouseButton,
    ) -> bool {
        crate::chrome::handle_mouse_click_on_chrome(self, ctx, button)
    }

    #[cfg(test)]
    pub(crate) fn handle_status_bar_click(&mut self, mouse_x: f32, mouse_y: f32) {
        crate::chrome::handle_status_bar_click(self, mouse_x, mouse_y);
    }

    fn command_n(&self, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed
            || self.modifiers.shift
            || self.modifiers.alt
            || self.ui.pending_chord.is_some()
        {
            return false;
        }
        let letter_n = match event.physical {
            Some(lux_platform::PhysicalKey::Letter(letter)) => letter.eq_ignore_ascii_case(&'n'),
            _ => matches!(&event.key, Key::Character(text) if text.eq_ignore_ascii_case("n")),
        };
        if !letter_n {
            return false;
        }
        #[cfg(target_os = "macos")]
        return self.modifiers.meta || crate::menu::macos::command_key_is_down();
        #[cfg(not(target_os = "macos"))]
        self.modifiers.control
    }

    pub(crate) fn handle_keyboard_event(
        &mut self,
        ctx: &dyn PlatformContext,
        key_event: &KeyEvent,
    ) {
        // A dead-key commit is queued before this key, and before the
        // modifiers for it are visible. Settle that commit first: a command
        // chord drops it, plain typing keeps it.
        let committed = crate::ime_hold::settle(self);
        // Key releases carry the same logical key as presses. They are useful
        // to the platform, but Lux has no release-triggered commands; routing
        // them through picker/search handlers used to insert every printable
        // key a second time.
        if key_event.state != ElementState::Pressed {
            return;
        }
        // Before a dead-key commit or a field can swallow the press.
        if self.command_n(key_event) {
            if crate::menu::claim_document_request(&crate::document::Request::New) {
                self.request_document(ctx, crate::document::Request::New);
            }
            return;
        }
        // The dead key already produced its commit. Typing the base key as
        // well would turn `á` into `áa` in the panel or the document.
        if committed {
            self.refresh_accessibility();
            return;
        }
        if crate::field::field_is_composing(self) {
            if matches!(key_event.key, Key::Named(NamedKey::Escape)) {
                crate::field::route_ime(self, &lux_platform::Ime::Disabled);
            }
            return;
        }
        if self.ui.settings.panel.is_some() && matches!(key_event.key, Key::Named(NamedKey::Escape))
        {
            let menu_open = self
                .ui
                .settings
                .panel
                .as_ref()
                .is_some_and(|panel| panel.menu.is_some());
            if menu_open {
                if let Some(panel) = self.ui.settings.panel.as_mut() {
                    panel.menu = None;
                }
            } else {
                self.ui.settings.panel = None;
                crate::focus::focus_editor(self);
            }
            self.scheduler.invalidate_all();
            self.refresh_accessibility();
            return;
        }
        if crate::field::edit_focused_field(self, key_event, ctx.clipboard()) {
            self.refresh_accessibility();
            return;
        }

        // Floating and focused surfaces own input before the document. This
        // prevents navigation/text intended for a modal from leaking into the
        // editor while still allowing an unhandled global shortcut to fall
        // through to the document/keymap dispatch below.
        if !crate::about::handle_about_key(self, ctx, key_event)
            && !crate::conflict::handle_conflict_key(self, ctx, key_event)
            && !crate::palette::handle_palette_key(self, ctx, key_event)
            && !crate::language_picker::handle_language_picker_key(self, ctx, key_event)
            && !crate::build::handle_build_key(self, ctx, key_event)
            && !crate::project::handle_project_switch_key(self, ctx, key_event)
            && !crate::theme_picker::handle_theme_picker_key(self, ctx, key_event)
            && !crate::snippets::handle_snippet_picker_key(self, ctx, key_event)
            && !crate::goto::handle_goto_key(self, ctx, key_event)
            && !crate::search::handle_search_key(self, ctx, key_event)
            && !crate::project_search::handle_project_search_key(self, ctx, key_event)
            && !crate::lsp::handle_completion_popup_key(self, key_event)
            && !crate::focus::handle_focus_key(self, ctx, key_event)
            && !self.document_key(ctx, key_event)
            && !crate::lsp::handle_lsp_key(self, ctx, key_event)
            && !crate::term::input::handle_term_key(self, ctx, key_event)
            && !crate::plugin::handle_plugin_key(self, ctx, key_event)
        {
            self.dispatch_key(key_event, Some(ctx.clipboard()), Some(ctx));
        }
        if self.is_dirty()
            && let Some(path) = &self.documents.path
        {
            self.ui.session.mark_dirty(path);
        }
        self.save_recovery_snapshot();
        self.update_document_title();
        if self.ui.announcement.is_none() {
            let head = self.cursors.primary().head;
            let marker = if self.document_dirty() {
                " (modified)"
            } else {
                ""
            };
            self.ui.announcement = Some(format!(
                "Ln {}, Col {}{marker}",
                head.line + 1,
                head.column + 1
            ));
        }
        self.refresh_accessibility();
    }
}

impl PlatformEventHandler for App {
    fn background_event(&mut self, ctx: &dyn PlatformContext) {
        crate::cli::dispatch(self, ctx);
        crate::git::poll(self);
        crate::session::poll_file_watcher(self);
        crate::menu::poll_and_dispatch(self, ctx);
        self.finish_document_operation(ctx);
        crate::lsp::poll_lsp_events(self, ctx);
        self.poll_syntax_highlighter();
        let mut settings = std::mem::take(&mut self.ui.settings);
        settings.poll_watcher(self);
        self.ui.settings = settings;
    }
    fn resumed(&mut self, ctx: &dyn PlatformContext) {
        crate::menu::register_waker(ctx.background_waker());
        crate::cli::register_waker(ctx.background_waker());
        crate::git::register_waker(ctx.background_waker());
        self.ui.session.register_waker(ctx.background_waker());
        if self.gpu_window.is_some() {
            return;
        }
        self.create_gpu_window(ctx);
        self.update_document_title();
        crate::menu::init_native_menu(self);
        if let Some(path) = self.documents.initial_path.take() {
            self.request_document(ctx, crate::document::Request::Open(Some(path)));
        } else {
            crate::document::open_queue::pump(self, ctx);
        }
        if !crate::document::open_queue::busy(self) && self.ui.sidebar.is_none() {
            let session_file = crate::session::default_session_path(None);
            if session_file.exists() {
                let _ = crate::session::restore_session(self, &session_file);
            }
        }
    }

    fn window_event(&mut self, ctx: &dyn PlatformContext, window: WindowId, event: PlatformEvent) {
        crate::menu::poll_and_dispatch(self, ctx);
        let Some(gpu_window) = &self.gpu_window else {
            return;
        };
        if gpu_window.id != window {
            return;
        }

        match event {
            PlatformEvent::CloseRequested => {
                crate::place::remember(self);
                ctx.exit();
            }
            PlatformEvent::Resized(size) => {
                self.resize(size);
                // Size and position come from the event. Querying the window
                // here calls into AppKit during `windowDidResize`, and
                // `is_maximized` can rewrite the style mask.
                crate::place::note_resized(self, size.width, size.height);
            }
            PlatformEvent::WindowMoved(position) => {
                crate::place::note_moved(self, position.x, position.y);
            }
            PlatformEvent::ScaleFactorChanged { scale_factor } => {
                self.set_display_scale(scale_factor);
                if let Some(gpu) = &self.gpu_window {
                    gpu.window.request_redraw();
                }
            }
            PlatformEvent::ColorSchemeChanged(scheme) => {
                self.apply_system_scheme(scheme);
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
                crate::session::poll_file_watcher(self);
                self.scheduler.invalidate_all();
                if let Some(gpu) = &self.gpu_window {
                    gpu.window.request_redraw();
                }
            }
            PlatformEvent::RedrawRequested => {
                let frame_began = Instant::now();
                self.redraw(ctx);
                crate::snapshot::capture_settled(self, ctx);
                crate::probe::on_frame(self, frame_began.elapsed());
            }
            PlatformEvent::Keyboard(key_event) => {
                if key_event.state == ElementState::Pressed
                    && key_event.text.as_ref().is_some_and(|text| !text.is_empty())
                {
                    crate::probe::note_key();
                }
                crate::probe::note_input(&key_event, &self.modifiers);
                self.handle_keyboard_event(ctx, &key_event);
                if let Some(gpu_window) = &self.gpu_window {
                    gpu_window.window.request_redraw();
                }
            }
            PlatformEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers,
            PlatformEvent::Focused(false) => {
                self.modifiers = Modifiers::default();
                self.mouse.selection_anchor = None;
            }
            PlatformEvent::Ime(ime_event) => {
                crate::ime_hold::observe(self, ime_event);
                self.refresh_accessibility();
                if let Some(gpu_window) = &self.gpu_window {
                    gpu_window.window.request_redraw();
                }
            }
            PlatformEvent::MouseWheel(delta) => {
                crate::probe::note_scroll();
                self.on_wheel(delta);
            }
            PlatformEvent::MouseMoved(position) => {
                self.mouse.position = position;
                self.mouse.placed = true;
                if crate::modal::handle_modal_hover(self, position.x as f32, position.y as f32)
                    && let Some(gpu_window) = &self.gpu_window
                {
                    gpu_window.window.request_redraw();
                }
                if crate::chrome::drag_tab(self)
                    && let Some(gpu_window) = &self.gpu_window
                {
                    gpu_window.window.request_redraw();
                }
                if self.ui.split.dragging_divider.is_some() {
                    self.handle_divider_drag(position.x as f32, position.y as f32);
                } else if self.mouse.selection_anchor.is_some() {
                    self.update_mouse_selection();
                    self.refresh_accessibility();
                    if let Some(gpu_window) = &self.gpu_window {
                        gpu_window.window.request_redraw();
                    }
                }
            }
            PlatformEvent::MouseButton { button, state } => {
                if state == ElementState::Pressed {
                    if !self.handle_mouse_click_on_chrome(ctx, button)
                        && button == MouseButton::Left
                    {
                        self.ui.project_search = None;
                        self.ui.command_palette = None;
                        self.ui.goto_anything = None;
                        self.ui.language_picker = None;
                        self.ui.build.picker = None;
                        self.ui.project.picker = None;
                        self.ui.theme_picker = None;
                        self.ui.snippets.picker = None;
                        crate::focus::focus_editor(self);

                        let pos = self.position_at_mouse();
                        let now = std::time::Instant::now();
                        let is_double_click = if let Some((last_time, last_line)) =
                            self.mouse.last_click
                        {
                            now.duration_since(last_time).as_millis() < 400 && last_line == pos.line
                        } else {
                            false
                        };
                        self.mouse.last_click = Some((now, pos.line));

                        if is_double_click && self.is_find_results_active() {
                            crate::find_results::open_find_result_at_line(
                                self,
                                Some(ctx),
                                pos.line,
                            );
                        } else {
                            self.begin_mouse_selection();
                        }
                    }
                } else if button == MouseButton::Left {
                    self.mouse.selection_anchor = None;
                    self.documents.tab_drag = None;
                    self.end_divider_drag();
                }
                self.refresh_accessibility();
                if let Some(gpu_window) = &self.gpu_window {
                    gpu_window.window.request_redraw();
                }
            }
            PlatformEvent::AccessibilityInitialTreeRequested => {
                self.accessibility_active = true;
                let update = self.build_accessibility_tree();
                if let Some(gpu_window) = &self.gpu_window {
                    gpu_window.window.update_accessibility_tree(update);
                }
            }
            PlatformEvent::AccessibilityActionRequested(request) => {
                if let Some(ActionData::SetTextSelection(selection)) = request.data
                    && let Some(selection) = self
                        .editor_view
                        .selection_from_accessibility(&self.buffer, selection)
                {
                    self.cursors = MultiCursor::new(selection);
                    self.reset_goal_columns();
                    self.refresh_accessibility();
                    if let Some(gpu_window) = &self.gpu_window {
                        gpu_window.window.request_redraw();
                    }
                }
            }
            PlatformEvent::AccessibilityDeactivated => self.accessibility_active = false,
            _ => {}
        }
    }

    fn about_to_wait(&mut self, ctx: &dyn PlatformContext) {
        // A menu key equivalent posts here and is otherwise drained on the
        // next window event. The probe snapshot is taken in this callback,
        // so the command has to land first.
        crate::menu::poll_and_dispatch(self, ctx);
        crate::session::poll_file_watcher(self);
        let mut settings = std::mem::take(&mut self.ui.settings);
        settings.poll_watcher(self);
        self.ui.settings = settings;
        crate::probe::poll(self, ctx);
        let waiting = crate::snapshot::usr1_waiting();
        let Some(gpu_window) = &self.gpu_window else {
            return;
        };
        if waiting
            || (self.presentation.ready(Instant::now())
                && self.scheduler.needs_repaint(Instant::now()))
        {
            gpu_window.window.request_redraw();
        }
    }

    fn exiting(&mut self, _ctx: &dyn PlatformContext) {
        self.sync_active_document_to_entry();
        crate::place::remember(self);
        let _ = crate::session::save_session(self, true);
        self.ui.term.shutdown_all();
    }
}
