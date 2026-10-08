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
use lux_platform::{
    ElementState, Ime, ImeComposition, ImeUpdate, Modifiers, MouseButton, PlatformContext,
    PlatformEvent, PlatformEventHandler, PlatformWindow, WindowId,
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
pub(crate) const TAB_BAR_HEIGHT: f32 = 32.0;
pub(crate) const TAB_MIN_WIDTH: f32 = 120.0;
pub(crate) const TAB_MAX_WIDTH: f32 = 200.0;
pub(crate) const TAB_CLOSE_BTN_WIDTH: f32 = 24.0;

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

pub(crate) struct MouseState {
    pub(crate) position: dpi::PhysicalPosition<f64>,
    pub(crate) selection_anchor: Option<Position>,
}

pub(crate) struct SyntaxState {
    pub(crate) document: Option<lux_syntax::Document>,
    pub(crate) source: String,
    pub(crate) theme: &'static lux_syntax::theme::SyntaxTheme,
}

pub(crate) struct App {
    pub(crate) scale_factor: f32,
    pub(crate) presentation: crate::presentation::Presentation,
    pub(crate) documents: crate::document::Documents,
    pub(crate) editor_view: EditorView,
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
    pub(crate) atlas: GlyphAtlas,
    pub(crate) atlas_dirty: bool,
    pub(crate) scale_cx: ScaleContext,
    pub(crate) gpu_window: Option<GpuWindow>,
    pub(crate) chrome: Chrome,
    pub(crate) keymap: lux_config::Keymap,
    pub(crate) snippet_session: Option<lux_core::snippets::SnippetSession>,
    pub(crate) syntax: Option<SyntaxState>,
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
            font,
            buffer,
            cursors,
            goal_columns: vec![0],
            history: History::new(),
            modifiers: Modifiers::default(),
            mouse: MouseState {
                position: dpi::PhysicalPosition::new(0.0, 0.0),
                selection_anchor: None,
            },
            accessibility_active: false,
            viewport: Viewport {
                width: 1000.0,
                height: 700.0 - TAB_BAR_HEIGHT,
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
            keymap: lux_config::keymap::presets::sublime(),
            snippet_session: None,
            syntax: None,
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

    pub(crate) fn refresh_accessibility(&mut self) {
        if !self.accessibility_active {
            return;
        }
        let update = self.build_accessibility_tree();
        if let Some(gpu_window) = &self.gpu_window {
            gpu_window.window.update_accessibility_tree(update);
        }
    }

    pub(crate) fn update_syntax_on_open(&mut self) {
        if self.buffer.is_huge_file_mode() {
            self.syntax = None;
            return;
        }
        let language = self.documents.path.as_ref().and_then(|p| {
            p.extension()
                .and_then(|ext| ext.to_str())
                .and_then(lux_syntax::LanguageId::from_extension)
        });
        if let Some(lang) = language {
            let source = self.buffer.text();
            let mut doc = lux_syntax::Document::new(lang);
            doc.set_text(&source);
            self.syntax = Some(SyntaxState {
                document: Some(doc),
                source,
                theme: &lux_syntax::theme::LUX_DARK,
            });
        } else {
            self.syntax = None;
        }
    }

    pub(crate) fn sync_syntax_on_edit(&mut self) {
        if self.buffer.is_huge_file_mode() {
            self.syntax = None;
            return;
        }
        if let Some(syntax) = &mut self.syntax {
            syntax.source = self.buffer.text();
            if let Some(doc) = &mut syntax.document {
                doc.set_text(&syntax.source);
            }
        }
    }

    pub(crate) fn tab_bar_height(&self) -> f32 {
        TAB_BAR_HEIGHT * self.scale_factor
    }

    pub(crate) fn tab_width(&self) -> f32 {
        let count = self.documents.tabs.tabs().len().max(1) as f32;
        let per_tab = self.viewport.width / count;
        per_tab.clamp(
            TAB_MIN_WIDTH * self.scale_factor,
            TAB_MAX_WIDTH * self.scale_factor,
        )
    }

    pub(crate) fn handle_mouse_click_on_chrome(
        &mut self,
        ctx: &dyn PlatformContext,
        button: MouseButton,
    ) -> bool {
        let mouse_y = self.mouse.position.y as f32;
        if mouse_y >= self.tab_bar_height() {
            return false;
        }
        let mouse_x = self.mouse.position.x as f32;
        let tab_w = self.tab_width();
        let tabs = self.documents.tabs.tabs().to_vec();
        let tab_idx = (mouse_x / tab_w) as usize;
        if let Some(tab) = tabs.get(tab_idx) {
            let tab_x = tab_idx as f32 * tab_w;
            let close_w = TAB_CLOSE_BTN_WIDTH * self.scale_factor;
            let on_close = mouse_x >= tab_x + tab_w - close_w;
            match button {
                MouseButton::Left => {
                    if on_close {
                        self.close_tab(ctx, tab.id);
                    } else {
                        self.switch_to_tab(tab.id);
                    }
                }
                MouseButton::Middle => {
                    self.close_tab(ctx, tab.id);
                }
                _ => {}
            }
        }
        true
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
                    self.handle_key_with_clipboard(&key_event, Some(ctx.clipboard()));
                }
                self.save_recovery_snapshot();
                self.update_document_title();
                self.refresh_accessibility();
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
                self.apply_ime(ime_event);
                self.refresh_accessibility();
                if let Some(gpu_window) = &self.gpu_window {
                    gpu_window.window.request_redraw();
                }
            }
            PlatformEvent::MouseWheel(delta) => {
                self.scroll(delta);
            }
            PlatformEvent::MouseMoved(position) => {
                self.mouse.position = position;
                if self.mouse.selection_anchor.is_some() {
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
                        self.begin_mouse_selection();
                    }
                } else if button == MouseButton::Left {
                    self.mouse.selection_anchor = None;
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

    fn about_to_wait(&mut self, _ctx: &dyn PlatformContext) {
        let Some(gpu_window) = &self.gpu_window else {
            return;
        };
        if self.presentation.ready(Instant::now()) && self.scheduler.needs_repaint(Instant::now()) {
            gpu_window.window.request_redraw();
        }
    }
}
