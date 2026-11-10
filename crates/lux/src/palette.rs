//! Unified Command Palette (G08): command registry, fuzzy search, execution, and state.

use std::collections::HashMap;

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext};
use lux_ui::command_palette::{CommandRegistry, Context, PaletteEntry};

pub(crate) struct CommandPaletteState {
    pub(crate) query: crate::field::QueryField,
    pub(crate) selected_index: usize,
    pub(crate) registry: CommandRegistry,
    pub(crate) shortcuts: HashMap<String, String>,
    pub(crate) motion: crate::overlay_motion::OverlayMotion,
    /// Theme active when the palette opened. Escape restores it.
    pub(crate) theme_before: Option<&'static str>,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPaletteState {
    pub(crate) fn new() -> Self {
        let mut registry = CommandRegistry::new();
        populate_registry(&mut registry);
        Self {
            query: crate::field::QueryField::new(),
            selected_index: 0,
            registry,
            shortcuts: HashMap::new(),
            motion: crate::overlay_motion::OverlayMotion::open(),
            theme_before: None,
        }
    }

    pub(crate) fn results<'a>(&'a self, ctx: &Context) -> Vec<PaletteEntry<'a>> {
        self.registry.search(&self.query, ctx)
    }

    pub(crate) fn select_next(&mut self, total: usize) {
        if total > 0 && self.selected_index + 1 < total {
            self.selected_index += 1;
        }
    }

    pub(crate) fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }
}

pub(crate) mod commands;
mod registry;
mod shortcuts;

pub(crate) use shortcuts::shortcut_labels;

fn populate_registry(registry: &mut CommandRegistry) {
    registry::populate_registry(registry);
}

fn palette_context(app: &crate::app::App) -> Context {
    let mut context = Context::new();
    context.set("editorFocus", true);
    context.set("hasSelection", !app.cursors.primary().is_empty());
    context
}

fn preview_palette_theme(app: &mut crate::app::App) {
    let selected = {
        let Some(palette) = app.ui.command_palette.as_ref() else {
            return;
        };
        let context = palette_context(app);
        let results = palette.results(&context);
        results.get(palette.selected_index).and_then(|entry| {
            entry
                .command
                .id
                .strip_prefix("theme.select:")
                .map(str::to_string)
        })
    };
    if let Some(name) = selected {
        app.preview_theme(&name);
        return;
    }
    if let Some(origin) = app
        .ui
        .command_palette
        .as_ref()
        .and_then(|palette| palette.theme_before)
    {
        app.set_theme(origin);
    }
}

fn restore_palette_theme(app: &mut crate::app::App) {
    if let Some(origin) = app
        .ui
        .command_palette
        .as_ref()
        .and_then(|palette| palette.theme_before)
    {
        app.set_theme(origin);
    }
}

pub(crate) fn handle_palette_key(
    app: &mut crate::app::App,
    ctx: &dyn PlatformContext,
    event: &KeyEvent,
) -> bool {
    if app.ui.command_palette.is_none() {
        return false;
    }
    let total = {
        let context = palette_context(app);
        app.ui
            .command_palette
            .as_ref()
            .map(|palette| palette.results(&context).len())
            .unwrap_or(0)
    };

    match &event.key {
        Key::Named(NamedKey::Escape) => {
            restore_palette_theme(app);
            app.ui.command_palette = None;
            crate::focus::focus_editor(app);
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowDown) => {
            if let Some(palette) = app.ui.command_palette.as_mut() {
                palette.select_next(total);
            }
            preview_palette_theme(app);
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowUp) => {
            if let Some(palette) = app.ui.command_palette.as_mut() {
                palette.select_prev();
            }
            preview_palette_theme(app);
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Backspace) => {
            if let Some(palette) = app.ui.command_palette.as_mut() {
                palette.query.pop();
                palette.selected_index = 0;
            }
            preview_palette_theme(app);
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Enter) => {
            let context = palette_context(app);
            let cmd_id = if let Some(palette) = app.ui.command_palette.as_mut() {
                let results = palette.results(&context);
                let cmd_id = results
                    .get(palette.selected_index)
                    .map(|entry| entry.command.id.clone());
                if let Some(cmd_id) = &cmd_id {
                    palette.registry.record_use(cmd_id);
                    let (uses, clock) = palette.registry.export_usage();
                    app.ui.settings.palette_uses = uses;
                    app.ui.settings.palette_clock = clock;
                }
                cmd_id
            } else {
                None
            };
            let keep = cmd_id
                .as_deref()
                .is_some_and(|id| id.starts_with("theme.select:"));
            if !keep {
                restore_palette_theme(app);
            }
            app.ui.command_palette = None;
            crate::focus::focus_editor(app);
            if let Some(cmd_id) = cmd_id {
                execute_command(app, ctx, &cmd_id);
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Character(s) => {
            if app.modifiers.meta || app.modifiers.control {
                return false;
            }
            if let Some(palette) = app.ui.command_palette.as_mut() {
                palette.query.push_str(s);
                palette.selected_index = 0;
            }
            preview_palette_theme(app);
            app.scheduler.invalidate_all();
            true
        }
        _ => false,
    }
}

pub(crate) fn execute_command(
    app: &mut crate::app::App,
    ctx: &dyn PlatformContext,
    command_id: &str,
) {
    commands::execute_command(app, ctx, command_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_initialization_has_registered_commands() {
        let palette = CommandPaletteState::new();
        let ctx = Context::new();
        let results = palette.results(&ctx);
        assert!(!results.is_empty());
        assert!(results.iter().any(|e| e.command.id == "file.save"));
        assert!(results.iter().any(|e| e.command.id == "edit.undo"));
    }

    #[test]
    fn palette_search_filters_by_query() {
        let mut palette = CommandPaletteState::new();
        let ctx = Context::new();
        palette.query = crate::field::QueryField::from_text("save");
        let results = palette.results(&ctx);
        assert!(!results.is_empty());
        assert!(results[0].command.title.to_lowercase().contains("save"));
    }

    #[test]
    fn palette_navigation_clamps_bounds() {
        let mut palette = CommandPaletteState::new();
        assert_eq!(palette.selected_index, 0);
        palette.select_prev();
        assert_eq!(palette.selected_index, 0);
        palette.select_next(5);
        assert_eq!(palette.selected_index, 1);
        palette.select_next(2);
        assert_eq!(palette.selected_index, 1);
    }

    #[test]
    fn app_palette_toggle_and_execution() {
        let font_bytes = crate::font::load_system_monospace_font()
            .map(|(_, b)| b)
            .unwrap();
        let mut app = crate::app::App::new(font_bytes);
        assert!(app.ui.command_palette.is_none());

        app.toggle_command_palette();
        assert!(app.ui.command_palette.is_some());

        let esc = KeyEvent {
            key: Key::Named(NamedKey::Escape),
            text: None,
            state: lux_platform::ElementState::Pressed,
            repeat: false,
            physical: None,
        };

        struct DummyDialogs;
        impl lux_platform::FileDialogs for DummyDialogs {
            fn pick_file(&self, _: &lux_platform::FileDialogOptions) -> Option<std::path::PathBuf> {
                None
            }
            fn pick_files(&self, _: &lux_platform::FileDialogOptions) -> Vec<std::path::PathBuf> {
                vec![]
            }
            fn pick_folder(
                &self,
                _: &lux_platform::FileDialogOptions,
            ) -> Option<std::path::PathBuf> {
                None
            }
            fn save_file(&self, _: &lux_platform::FileDialogOptions) -> Option<std::path::PathBuf> {
                None
            }
            fn confirm_save(&self, _: &str) -> lux_platform::dialog::SaveDecision {
                lux_platform::dialog::SaveDecision::Discard
            }
        }
        static DUMMY_DIALOGS: DummyDialogs = DummyDialogs;

        struct DummyContext;
        impl lux_platform::Clipboard for DummyContext {
            fn read_text(&self) -> Result<String, lux_platform::PlatformError> {
                Ok(String::new())
            }
            fn write_text(&self, _: &str) -> Result<(), lux_platform::PlatformError> {
                Ok(())
            }
        }
        impl PlatformContext for DummyContext {
            fn background_waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync> {
                std::sync::Arc::new(|| {})
            }
            fn background_dialogs(&self) -> std::sync::Arc<dyn lux_platform::FileDialogs> {
                std::sync::Arc::new(DummyDialogs)
            }
            fn file_dialogs(&self) -> &dyn lux_platform::FileDialogs {
                &DUMMY_DIALOGS
            }
            fn clipboard(&self) -> &dyn lux_platform::Clipboard {
                self
            }
            fn create_window(
                &self,
                _: lux_platform::WindowAttributes,
            ) -> Result<Box<dyn lux_platform::PlatformWindow>, lux_platform::PlatformError>
            {
                panic!("unused")
            }
            fn request_wakeup(&self, _: std::time::Instant) {}
            fn exit(&self) {}
        }
        let ctx = DummyContext;
        let handled = handle_palette_key(&mut app, &ctx, &esc);
        assert!(handled);
        assert!(app.ui.command_palette.is_none());

        // Re-open and execute toggle EOL command
        app.toggle_command_palette();
        let eol_before = app.buffer.eol();
        execute_command(&mut app, &ctx, "view.toggle_line_endings");
        assert_ne!(app.buffer.eol(), eol_before);
    }
}
