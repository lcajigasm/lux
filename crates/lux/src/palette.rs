//! Unified Command Palette (G08): command registry, fuzzy search, execution, and state.

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext};
use lux_ui::command_palette::{CommandRegistry, Context, PaletteEntry};

pub(crate) struct CommandPaletteState {
    pub(crate) query: String,
    pub(crate) selected_index: usize,
    pub(crate) registry: CommandRegistry,
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
            query: String::new(),
            selected_index: 0,
            registry,
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

fn populate_registry(registry: &mut CommandRegistry) {
    registry::populate_registry(registry);
}

pub(crate) fn handle_palette_key(
    app: &mut crate::app::App,
    ctx: &dyn PlatformContext,
    event: &KeyEvent,
) -> bool {
    let Some(palette) = app.ui.command_palette.as_mut() else {
        return false;
    };

    let mut context = Context::new();
    context.set("editorFocus", true);
    context.set("hasSelection", !app.cursors.primary().is_empty());
    let results = palette.results(&context);
    let total = results.len();

    match &event.key {
        Key::Named(NamedKey::Escape) => {
            app.ui.command_palette = None;
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowDown) => {
            palette.select_next(total);
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowUp) => {
            palette.select_prev();
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Backspace) => {
            palette.query.pop();
            palette.selected_index = 0;
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Enter) => {
            if let Some(entry) = results.get(palette.selected_index) {
                let cmd_id = entry.command.id.clone();
                palette.registry.record_use(&cmd_id);
                app.ui.command_palette = None;
                execute_command(app, ctx, &cmd_id);
            } else {
                app.ui.command_palette = None;
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Character(s) => {
            if !app.modifiers.meta && !app.modifiers.control {
                palette.query.push_str(s);
                palette.selected_index = 0;
                app.scheduler.invalidate_all();
                return true;
            }
            false
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
        palette.query = "save".to_string();
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
