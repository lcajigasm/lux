//! Color Scheme / Theme Picker modal dialog (ST15).

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext};

use crate::app::App;

#[derive(Debug, Clone)]
pub(crate) struct ThemePickerState {
    pub(crate) query: String,
    pub(crate) selected_index: usize,
    pub(crate) original_theme_name: &'static str,
    pub(crate) candidates: Vec<&'static str>,
}

pub(crate) fn all_theme_names() -> Vec<&'static str> {
    vec![
        "Monokai",
        "Mariana",
        "Breakers",
        "Celeste",
        "Lux Dark",
        "Lux Light",
        "Lux High Contrast",
    ]
}

impl ThemePickerState {
    pub(crate) fn new(current_theme_name: &'static str) -> Self {
        let candidates = all_theme_names();
        let selected_index = candidates
            .iter()
            .position(|&c| c.eq_ignore_ascii_case(current_theme_name))
            .unwrap_or(0);
        Self {
            query: String::new(),
            selected_index,
            original_theme_name: current_theme_name,
            candidates,
        }
    }

    pub(crate) fn filtered(&self) -> Vec<&'static str> {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() {
            self.candidates.clone()
        } else {
            self.candidates
                .iter()
                .copied()
                .filter(|name| name.to_lowercase().contains(&q))
                .collect()
        }
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

    pub(crate) fn current_candidate(&self) -> Option<&'static str> {
        let list = self.filtered();
        list.get(self.selected_index).copied()
    }
}

pub(crate) fn handle_theme_picker_key(
    app: &mut App,
    _ctx: &dyn PlatformContext,
    event: &KeyEvent,
) -> bool {
    let Some(picker) = &app.ui.theme_picker else {
        return false;
    };

    let original = picker.original_theme_name;
    let filtered_len = picker.filtered().len();

    match &event.key {
        Key::Named(NamedKey::Escape) => {
            app.set_theme(original);
            app.ui.theme_picker = None;
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Enter) => {
            if let Some(picker) = app.ui.theme_picker.as_ref()
                && let Some(candidate) = picker.current_candidate()
            {
                app.confirm_theme(candidate);
            }
            app.ui.theme_picker = None;
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowDown) => {
            if let Some(picker) = app.ui.theme_picker.as_mut() {
                picker.select_next(filtered_len);
                if let Some(candidate) = picker.current_candidate() {
                    app.preview_theme(candidate);
                }
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowUp) => {
            if let Some(picker) = app.ui.theme_picker.as_mut() {
                picker.select_prev();
                if let Some(candidate) = picker.current_candidate() {
                    app.preview_theme(candidate);
                }
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Backspace) => {
            if let Some(picker) = app.ui.theme_picker.as_mut() {
                picker.query.pop();
                picker.selected_index = 0;
                if let Some(candidate) = picker.current_candidate() {
                    app.preview_theme(candidate);
                }
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Character(text) if !text.chars().any(char::is_control) => {
            if let Some(picker) = app.ui.theme_picker.as_mut() {
                picker.query.push_str(text);
                picker.selected_index = 0;
                if let Some(candidate) = picker.current_candidate() {
                    app.preview_theme(candidate);
                }
            }
            app.scheduler.invalidate_all();
            true
        }
        _ => false,
    }
}

impl App {
    pub(crate) fn active_syntax_theme(&self) -> &'static lux_syntax::theme::SyntaxTheme {
        lux_syntax::theme::by_name(self.chrome.theme.name).unwrap_or(&lux_syntax::theme::LUX_DARK)
    }

    pub(crate) fn set_theme(&mut self, name: &str) {
        if let Some(ui_theme) = lux_ui::Theme::by_name(name) {
            self.chrome.theme = ui_theme;
        }
        if let Some(syntax_theme) = lux_syntax::theme::by_name(name)
            && let Some(syntax) = &mut self.syntax
        {
            syntax.theme = syntax_theme;
        }
        self.sync_editor_clear_color();
        self.scheduler.invalidate_all();
        if let Some(gpu) = &self.gpu_window {
            gpu.window.request_redraw();
        }
    }

    /// Named themes stick. `system` / `auto` follow `chrome.system_scheme`.
    pub(crate) fn apply_color_theme_setting(&mut self, name: &str) {
        if lux_ui::Chrome::follows_system(name) {
            self.chrome.theme_follows_system = true;
            self.set_theme(lux_ui::Chrome::theme_name_for_scheme(
                self.chrome.system_scheme,
            ));
        } else {
            self.chrome.theme_follows_system = false;
            self.set_theme(name);
        }
    }

    pub(crate) fn apply_system_scheme(&mut self, scheme: lux_platform::ColorScheme) {
        self.chrome.system_scheme = match scheme {
            lux_platform::ColorScheme::Dark => lux_ui::SystemScheme::Dark,
            lux_platform::ColorScheme::Light => lux_ui::SystemScheme::Light,
        };
        if self.chrome.theme_follows_system {
            self.set_theme(lux_ui::Chrome::theme_name_for_scheme(
                self.chrome.system_scheme,
            ));
        }
    }

    pub(crate) fn apply_saved_color_theme(&mut self) {
        let name = {
            let config = self.ui.settings.config.lock().unwrap();
            config.resolved()["workbench.colorTheme"]
                .as_str()
                .unwrap_or("system")
                .to_string()
        };
        self.apply_color_theme_setting(&name);
    }

    pub(crate) fn preview_theme(&mut self, name: &str) {
        self.set_theme(name);
    }

    pub(crate) fn confirm_theme(&mut self, name: &str) {
        self.apply_color_theme_setting(name);
        let mut settings = std::mem::take(&mut self.ui.settings);
        settings.set_string_setting("workbench.colorTheme", name);
        self.ui.settings = settings;

        let display_name = self.chrome.theme.name;
        crate::accessibility::announce(self, format!("Color theme set to {display_name}"));
    }

    pub(crate) fn open_theme_picker(&mut self) {
        let current_theme = self.chrome.theme.name;
        let picker = ThemePickerState::new(current_theme);
        self.ui.theme_picker = Some(picker);
        self.scheduler.invalidate_all();
    }

    #[allow(dead_code)]
    pub(crate) fn close_theme_picker(&mut self) {
        if let Some(picker) = self.ui.theme_picker.take() {
            self.set_theme(picker.original_theme_name);
        }
        self.scheduler.invalidate_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lux_platform::ColorScheme;
    use lux_ui::Theme;

    fn app() -> App {
        App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec())
    }

    #[test]
    fn system_light_uses_lux_light_until_a_named_theme_is_chosen() {
        let mut app = app();
        app.apply_system_scheme(ColorScheme::Light);
        assert_eq!(app.chrome.theme.name, Theme::LUX_LIGHT.name);
        assert_ne!(app.chrome.theme.background, Theme::LUX_DARK.background);
        app.apply_color_theme_setting("lux-dark");
        assert!(!app.chrome.theme_follows_system);
        assert_eq!(app.chrome.theme.name, Theme::LUX_DARK.name);
        app.apply_system_scheme(ColorScheme::Light);
        assert_eq!(app.chrome.theme.name, Theme::LUX_DARK.name);
        app.apply_color_theme_setting("system");
        assert!(app.chrome.theme_follows_system);
        assert_eq!(app.chrome.theme.name, Theme::LUX_LIGHT.name);
    }
}
