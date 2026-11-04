//! Real Language Server Protocol client integration (Goal G16).
//! Powered by `lux-lsp` (`async-lsp`-backed, zero UI thread blocking).

mod actions;
#[cfg(test)]
mod tests;

pub(crate) use actions::{
    find_references, format_document, goto_definition, jump_to_location, request_hover,
};

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

use lux_lsp::async_lsp::lsp_types::{
    CompletionItem, DiagnosticSeverity, Location, Position, Range, TextEdit, Url,
};
use lux_lsp::{DocumentSync, ServerRegistry, ServerSession};
use lux_platform::event::{ElementState, KeyEvent};
use lux_platform::{Key, NamedKey, PlatformContext};
use lux_syntax::LanguageId;

use crate::app::App;

#[derive(Debug, Clone)]
pub(crate) struct DiagnosticItem {
    pub(crate) range: Range,
    pub(crate) severity: Option<DiagnosticSeverity>,
    pub(crate) message: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CompletionPopupState {
    pub(crate) items: Vec<CompletionItem>,
    pub(crate) selected_index: usize,
    pub(crate) rect: [f32; 4],
    pub(crate) motion: crate::overlay_motion::OverlayMotion,
}

impl CompletionPopupState {
    pub(crate) fn showing(items: Vec<CompletionItem>) -> Self {
        Self {
            items,
            selected_index: 0,
            rect: [0.0; 4],
            motion: crate::overlay_motion::OverlayMotion::open(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HoverPopupState {
    pub(crate) text: String,
    pub(crate) rect: [f32; 4],
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ProblemsPanelState {
    pub(crate) visible: bool,
    pub(crate) selected_index: usize,
    pub(crate) rect: [f32; 4],
    pub(crate) focused: bool,
}

pub(crate) enum LspEvent {
    DiagnosticsUpdated {
        url: Url,
        diagnostics: Vec<DiagnosticItem>,
    },
    CompletionsReady {
        items: Vec<CompletionItem>,
    },
    HoverReady {
        text: Option<String>,
    },
    DefinitionReady {
        locations: Vec<Location>,
    },
    ReferencesReady {
        locations: Vec<Location>,
    },
    FormattingReady {
        edits: Vec<TextEdit>,
    },
}

pub(crate) struct LspState {
    pub(crate) registry: Arc<ServerRegistry>,
    pub(crate) sync: Option<Arc<DocumentSync>>,
    pub(crate) current_session: Option<Arc<ServerSession>>,
    pub(crate) current_url: Option<Url>,
    pub(crate) diagnostics: Vec<DiagnosticItem>,
    pub(crate) all_diagnostics: Vec<(Url, Vec<DiagnosticItem>)>,
    pub(crate) completion_popup: Option<CompletionPopupState>,
    pub(crate) hover_popup: Option<HoverPopupState>,
    pub(crate) problems: ProblemsPanelState,
    pub(crate) sender: Sender<LspEvent>,
    pub(crate) receiver: Receiver<LspEvent>,
    pub(crate) active_sync_sessions: Arc<Mutex<Vec<Arc<ServerSession>>>>,
}

impl Default for LspState {
    fn default() -> Self {
        Self::new()
    }
}

impl LspState {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = channel();
        Self {
            registry: Arc::new(ServerRegistry::new()),
            sync: None,
            current_session: None,
            current_url: None,
            diagnostics: Vec::new(),
            all_diagnostics: Vec::new(),
            completion_popup: None,
            hover_popup: None,
            problems: ProblemsPanelState::default(),
            sender,
            receiver,
            active_sync_sessions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
            .count()
    }

    #[allow(dead_code)]
    pub(crate) fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Some(DiagnosticSeverity::WARNING))
            .count()
    }
}

pub(crate) fn jump_to_next_diagnostic(app: &mut App) -> bool {
    if app.ui.lsp.diagnostics.is_empty() {
        return false;
    }
    let cur_line = app.cursors.primary().head.line as u32;
    let next_diag = app
        .ui
        .lsp
        .diagnostics
        .iter()
        .find(|d| d.range.start.line > cur_line)
        .or_else(|| app.ui.lsp.diagnostics.first())
        .cloned();

    if let Some(d) = next_diag {
        crate::goto::jump_to_line(app, d.range.start.line + 1);
        app.documents.message = Some(format!("Problem: {}", d.message));
        app.scheduler.invalidate_all();
        return true;
    }
    false
}

pub(crate) fn jump_to_prev_diagnostic(app: &mut App) -> bool {
    if app.ui.lsp.diagnostics.is_empty() {
        return false;
    }
    let cur_line = app.cursors.primary().head.line as u32;
    let prev_diag = app
        .ui
        .lsp
        .diagnostics
        .iter()
        .rfind(|d| d.range.start.line < cur_line)
        .or_else(|| app.ui.lsp.diagnostics.last())
        .cloned();

    if let Some(d) = prev_diag {
        crate::goto::jump_to_line(app, d.range.start.line + 1);
        app.documents.message = Some(format!("Problem: {}", d.message));
        app.scheduler.invalidate_all();
        return true;
    }
    false
}

pub(crate) fn lsp_language_for_path(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?;
    let lang = LanguageId::from_extension(ext)?;
    match lang {
        LanguageId::Rust => Some("rust"),
        LanguageId::TypeScript | LanguageId::Tsx | LanguageId::JavaScript => Some("typescript"),
        LanguageId::Python => Some("python"),
        LanguageId::Go => Some("go"),
        LanguageId::C | LanguageId::Cpp => Some("c"),
        _ => None,
    }
}

pub(crate) fn sync_document_open(
    app: &mut App,
    ctx: &dyn PlatformContext,
    path: &Path,
    text: &str,
) {
    if app.ui.settings.is_settings_path(path) {
        let settings = std::mem::take(&mut app.ui.settings);
        settings.validate_buffer(app, path, text);
        app.ui.settings = settings;
    }

    let Some(lang) = lsp_language_for_path(path) else {
        app.ui.lsp.current_session = None;
        app.ui.lsp.sync = None;
        app.ui.lsp.current_url = None;
        if !app.ui.settings.is_settings_path(path) {
            app.ui.lsp.diagnostics.clear();
        }
        return;
    };

    let Ok(url) = Url::from_file_path(path) else {
        return;
    };
    app.ui.lsp.current_url = Some(url.clone());

    let root = app
        .ui
        .sidebar
        .as_ref()
        .and_then(|s| s.workspace_root.clone())
        .or_else(|| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));

    let registry = app.ui.lsp.registry.clone();
    let sender = app.ui.lsp.sender.clone();
    let waker = ctx.background_waker();
    let text = text.to_string();
    let active_sync_sessions = app.ui.lsp.active_sync_sessions.clone();

    thread::Builder::new()
        .name("lux-lsp-spawn".into())
        .spawn(move || {
            let session = pollster::block_on(registry.get_or_start(lang, &root));
            let Ok(session) = session else {
                return;
            };

            let sync = DocumentSync::new(session.clone());
            let _ = sync.open(url.clone(), lang, text);

            let mut sessions = active_sync_sessions.lock().unwrap();
            if !sessions.iter().any(|s| Arc::ptr_eq(s, &session)) {
                sessions.push(session.clone());

                let sess_clone = session.clone();
                let sender_clone = sender.clone();
                let waker_clone = waker.clone();
                thread::Builder::new()
                    .name("lux-lsp-diagnostics".into())
                    .spawn(move || {
                        loop {
                            thread::sleep(std::time::Duration::from_millis(150));
                            let diags = lux_lsp::Diagnostics::new(sess_clone.clone()).all();
                            let mut by_url: std::collections::HashMap<Url, Vec<DiagnosticItem>> =
                                std::collections::HashMap::new();
                            for (doc_url, d) in diags {
                                by_url.entry(doc_url).or_default().push(DiagnosticItem {
                                    range: d.range,
                                    severity: d.severity,
                                    message: d.message,
                                });
                            }
                            for (doc_url, items) in by_url {
                                if sender_clone
                                    .send(LspEvent::DiagnosticsUpdated {
                                        url: doc_url,
                                        diagnostics: items,
                                    })
                                    .is_ok()
                                {
                                    waker_clone();
                                }
                            }
                            if sess_clone.is_terminal() {
                                break;
                            }
                        }
                    })
                    .ok();
            }
        })
        .ok();
}

pub(crate) fn sync_document_change(app: &mut App, path: &Path, text: &str) {
    if app.ui.settings.is_settings_path(path) {
        let settings = std::mem::take(&mut app.ui.settings);
        settings.validate_buffer(app, path, text);
        app.ui.settings = settings;
    }
    let Some(sync) = &app.ui.lsp.sync else {
        return;
    };
    let Ok(url) = Url::from_file_path(path) else {
        return;
    };
    let _ = sync.change(
        &url,
        vec![lux_lsp::TextEdit {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: u32::MAX,
                    character: u32::MAX,
                },
            },
            new_text: text.to_string(),
        }],
    );
}

pub(crate) fn sync_document_save(app: &mut App, path: &Path) {
    if app.ui.settings.is_settings_path(path) {
        let _ = app
            .ui
            .settings
            .config
            .lock()
            .unwrap()
            .reload_layer(lux_config::layering::Layer::User);
        let settings = std::mem::take(&mut app.ui.settings);
        settings.apply_to_app(app);
        app.ui.settings = settings;
    }
    let Some(sync) = &app.ui.lsp.sync else {
        return;
    };
    let Ok(url) = Url::from_file_path(path) else {
        return;
    };
    let _ = sync.save(&url);
}

pub(crate) fn poll_lsp_events(app: &mut App, _ctx: &dyn PlatformContext) {
    let mut needs_redraw = false;

    while let Ok(event) = app.ui.lsp.receiver.try_recv() {
        match event {
            LspEvent::DiagnosticsUpdated { url, diagnostics } => {
                if app.ui.lsp.current_url.as_ref() == Some(&url) {
                    app.ui.lsp.diagnostics = diagnostics.clone();
                    needs_redraw = true;
                }
                if let Some(entry) = app
                    .ui
                    .lsp
                    .all_diagnostics
                    .iter_mut()
                    .find(|(u, _)| u == &url)
                {
                    entry.1 = diagnostics;
                } else {
                    app.ui.lsp.all_diagnostics.push((url, diagnostics));
                }
            }
            LspEvent::CompletionsReady { items } => {
                if !items.is_empty() {
                    app.ui.lsp.completion_popup = Some(CompletionPopupState::showing(items));
                    needs_redraw = true;
                }
            }
            LspEvent::HoverReady { text } => {
                if let Some(t) = text {
                    app.ui.lsp.hover_popup = Some(HoverPopupState {
                        text: t,
                        rect: [0.0; 4],
                    });
                    needs_redraw = true;
                } else {
                    app.ui.lsp.hover_popup = None;
                }
            }
            LspEvent::DefinitionReady { locations } => {
                if let Some(loc) = locations.first()
                    && let Ok(target_path) = loc.uri.to_file_path()
                {
                    actions::jump_to_location(app, &target_path, loc.range.start.line as usize);
                    needs_redraw = true;
                }
            }
            LspEvent::ReferencesReady { locations } => {
                if let Some(loc) = locations.first()
                    && let Ok(target_path) = loc.uri.to_file_path()
                {
                    actions::jump_to_location(app, &target_path, loc.range.start.line as usize);
                    needs_redraw = true;
                }
            }
            LspEvent::FormattingReady { edits } => {
                actions::apply_lsp_edits(app, &edits);
                needs_redraw = true;
            }
        }
    }

    if needs_redraw {
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
    }
}

pub(crate) fn handle_completion_popup_key(app: &mut App, event: &KeyEvent) -> bool {
    if event.state != ElementState::Pressed {
        return false;
    }
    if let Some(popup) = app.ui.lsp.completion_popup.as_mut() {
        match &event.key {
            Key::Named(NamedKey::ArrowDown) => {
                if popup.selected_index + 1 < popup.items.len() {
                    popup.selected_index += 1;
                    app.scheduler.invalidate_all();
                    return true;
                }
            }
            Key::Named(NamedKey::ArrowUp) => {
                if popup.selected_index > 0 {
                    popup.selected_index -= 1;
                    app.scheduler.invalidate_all();
                    return true;
                }
            }
            Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Tab) => {
                if let Some(item) = popup.items.get(popup.selected_index).cloned() {
                    let insert_text = item.insert_text.unwrap_or_else(|| item.label.clone());
                    app.insert_at_cursor(&insert_text);
                    app.ui.lsp.completion_popup = None;
                    app.scheduler.invalidate_all();
                    return true;
                }
            }
            Key::Named(NamedKey::Escape) => {
                app.ui.lsp.completion_popup = None;
                app.scheduler.invalidate_all();
                return true;
            }
            _ => {}
        }
    }
    false
}

pub(crate) fn handle_lsp_key(app: &mut App, ctx: &dyn PlatformContext, event: &KeyEvent) -> bool {
    if event.state != ElementState::Pressed {
        return false;
    }
    if handle_completion_popup_key(app, event) {
        return true;
    }

    // Hover popup dismissal
    if app.ui.lsp.hover_popup.is_some() && matches!(&event.key, Key::Named(NamedKey::Escape)) {
        app.ui.lsp.hover_popup = None;
        app.scheduler.invalidate_all();
        return true;
    }

    // 3. LSP shortcuts
    match &event.key {
        Key::Named(NamedKey::F(12)) => {
            if app.modifiers.shift {
                actions::find_references(app, ctx);
            } else {
                actions::goto_definition(app, ctx);
            }
            true
        }
        Key::Named(NamedKey::Space) if app.modifiers.control => {
            actions::request_completion(app, ctx);
            true
        }
        Key::Named(NamedKey::F(8)) => {
            if app.modifiers.shift {
                jump_to_prev_diagnostic(app);
            } else {
                jump_to_next_diagnostic(app);
            }
            app.ui.lsp.problems.visible = true;
            app.scheduler.invalidate_all();
            true
        }
        _ => false,
    }
}
