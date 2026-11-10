//! Preferences page: the setting list and the controls that change it.

use lux_platform::event::{ElementState, KeyEvent};
use lux_platform::{Key, NamedKey, ScrollDelta};
use serde_json::Value;

use super::SettingsState;
use super::layout::{SettingsBounds, SettingsHit, layout_settings};
use crate::app::App;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SettingKind {
    Number {
        value: f64,
        options: &'static [f64],
    },
    Boolean {
        value: bool,
    },
    Enum {
        value: String,
        options: &'static [&'static str],
    },
}

#[derive(Debug, Clone)]
pub(crate) struct SettingItem {
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) description: &'static str,
    pub(crate) kind: SettingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SettingsMenu {
    pub(crate) item: usize,
    pub(crate) highlight: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SettingsPanelState {
    pub(crate) selected_index: usize,
    pub(crate) items: Vec<SettingItem>,
    pub(crate) scroll: f32,
    pub(crate) menu: Option<SettingsMenu>,
    /// Theme name and `theme_follows_system` from before this menu previewed.
    pub(crate) theme_before: Option<(String, bool)>,
}

enum Edit {
    SetBool(bool),
    Step(bool),
    Choose(usize),
}

impl SettingsState {
    pub(crate) fn open_settings_ui(&mut self) {
        if self.panel.is_some() {
            self.panel = None;
            return;
        }

        let config = self.config.lock().unwrap();
        let items = super::catalog::page_items(config.resolved());
        drop(config);

        self.panel = Some(SettingsPanelState {
            selected_index: 0,
            items,
            scroll: 0.0,
            menu: None,
            theme_before: None,
        });
    }

    pub(crate) fn update_setting_value(&mut self, app: &mut App, key: &str, val: Value) {
        let mut map: serde_json::Map<String, Value> = match fs_read_map(&self.settings_path) {
            Some(map) => map,
            None => serde_json::Map::new(),
        };
        map.insert(key.to_string(), val);
        let obj = Value::Object(map);
        if let Ok(json_str) = serde_json::to_string_pretty(&obj) {
            let _ = std::fs::write(&self.settings_path, json_str);
        }
        let _ = self
            .config
            .lock()
            .unwrap()
            .reload_layer(lux_config::layering::Layer::User);
        self.apply_to_app(app);
    }

    pub(crate) fn handle_key(&mut self, app: &mut App, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed || self.panel.is_none() {
            return false;
        }
        let bounds = bounds_for(app);
        match &event.key {
            Key::Named(NamedKey::Escape) => {
                let menu_open = self
                    .panel
                    .as_ref()
                    .is_some_and(|panel| panel.menu.is_some());
                if menu_open {
                    self.dismiss_menu(app, false);
                } else {
                    self.panel = None;
                }
                app.scheduler.invalidate_all();
                true
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.move_selection(bounds, 1);
                self.preview_menu_theme(app);
                app.scheduler.invalidate_all();
                true
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.move_selection(bounds, -1);
                self.preview_menu_theme(app);
                app.scheduler.invalidate_all();
                true
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.nudge_selected(app, true);
                true
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.nudge_selected(app, false);
                true
            }
            Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
                self.activate_selected(app);
                true
            }
            _ => true,
        }
    }

    pub(crate) fn handle_click(&mut self, app: &mut App, mouse_x: f32, mouse_y: f32) -> bool {
        let Some(panel) = self.panel.as_ref() else {
            return false;
        };
        let bounds = bounds_for(app);
        let layout = layout_settings(panel, bounds);
        match layout.hit(mouse_x, mouse_y) {
            SettingsHit::Outside => {
                self.panel = None;
                crate::focus::focus_editor(app);
                app.scheduler.invalidate_all();
            }
            SettingsHit::Page => {
                self.dismiss_menu(app, false);
                app.scheduler.invalidate_all();
            }
            SettingsHit::Row(index) => {
                self.dismiss_menu(app, false);
                self.select(index);
                app.scheduler.invalidate_all();
            }
            SettingsHit::Toggle(index) => {
                let on = self.panel.as_ref().is_some_and(|panel| {
                    matches!(
                        panel.items.get(index).map(|item| &item.kind),
                        Some(SettingKind::Boolean { value: true })
                    )
                });
                self.select(index);
                self.edit(app, index, Edit::SetBool(!on));
            }
            SettingsHit::StepMinus(index) => {
                self.select(index);
                self.edit(app, index, Edit::Step(false));
            }
            SettingsHit::StepPlus(index) => {
                self.select(index);
                self.edit(app, index, Edit::Step(true));
            }
            SettingsHit::MenuButton(index) => self.toggle_menu(app, index),
            SettingsHit::MenuOption { item, option } => {
                self.edit(app, item, Edit::Choose(option));
                self.dismiss_menu(app, true);
                if let Some(panel) = self.panel.as_mut() {
                    panel.selected_index = item;
                }
            }
        }
        true
    }

    pub(crate) fn scroll_by(&mut self, delta: ScrollDelta, scale: f32) -> bool {
        let Some(panel) = self.panel.as_mut() else {
            return false;
        };
        let dy = match delta {
            ScrollDelta::Lines { y, .. } => -y * 48.0 * scale.max(1.0),
            ScrollDelta::Pixels(position) => -(position.y as f32),
        };
        panel.scroll = (panel.scroll + dy).max(0.0);
        true
    }

    fn move_selection(&mut self, bounds: SettingsBounds, delta: isize) {
        let Some(panel) = self.panel.as_mut() else {
            return;
        };
        if panel.menu.is_some() {
            let len = panel
                .menu
                .and_then(|menu| match &panel.items.get(menu.item)?.kind {
                    SettingKind::Enum { options, .. } => Some(options.len()),
                    _ => None,
                })
                .unwrap_or(0);
            if let Some(menu) = panel.menu.as_mut() {
                if delta < 0 {
                    menu.highlight = menu.highlight.saturating_sub(delta.unsigned_abs());
                } else if len > 0 {
                    menu.highlight = (menu.highlight + delta as usize).min(len - 1);
                }
            }
            return;
        }
        if panel.items.is_empty() {
            return;
        }
        let len = panel.items.len() as isize;
        let next = (panel.selected_index as isize + delta).clamp(0, len - 1) as usize;
        panel.selected_index = next;
        reveal(panel, bounds);
    }

    fn nudge_selected(&mut self, app: &mut App, forward: bool) {
        let Some(index) = self.panel.as_ref().map(|panel| panel.selected_index) else {
            return;
        };
        self.dismiss_menu(app, false);
        let edit = match self.panel.as_ref().and_then(|panel| panel.items.get(index)) {
            Some(SettingItem {
                kind: SettingKind::Boolean { .. },
                ..
            }) => Edit::SetBool(forward),
            _ => Edit::Step(forward),
        };
        self.edit(app, index, edit);
    }

    fn activate_selected(&mut self, app: &mut App) {
        let Some(panel) = self.panel.as_ref() else {
            return;
        };
        if let Some(menu) = panel.menu {
            let option = menu.highlight;
            let item = menu.item;
            self.edit(app, item, Edit::Choose(option));
            self.dismiss_menu(app, true);
            return;
        }
        let index = panel.selected_index;
        match panel.items.get(index).map(|item| &item.kind) {
            Some(SettingKind::Boolean { value }) => {
                let next = !*value;
                self.edit(app, index, Edit::SetBool(next));
            }
            Some(SettingKind::Enum { .. }) => self.toggle_menu(app, index),
            Some(SettingKind::Number { .. }) => self.edit(app, index, Edit::Step(true)),
            None => {}
        }
    }

    fn toggle_menu(&mut self, app: &mut App, index: usize) {
        let open = self
            .panel
            .as_ref()
            .is_some_and(|panel| panel.menu.is_some_and(|menu| menu.item == index));
        if open {
            self.dismiss_menu(app, false);
            return;
        }
        let Some(panel) = self.panel.as_mut() else {
            return;
        };
        panel.selected_index = index;
        let highlight = match panel.items.get(index).map(|item| &item.kind) {
            Some(SettingKind::Enum { value, options }) => options
                .iter()
                .position(|option| option == value)
                .unwrap_or(0),
            _ => return,
        };
        panel.menu = Some(SettingsMenu {
            item: index,
            highlight,
        });
        self.preview_menu_theme(app);
    }

    fn select(&mut self, index: usize) {
        if let Some(panel) = self.panel.as_mut() {
            panel.selected_index = index;
        }
    }

    fn color_theme_menu_open(&self) -> bool {
        self.panel.as_ref().is_some_and(|panel| {
            panel.menu.is_some_and(|menu| {
                panel
                    .items
                    .get(menu.item)
                    .is_some_and(|item| item.key == "workbench.colorTheme")
            })
        })
    }

    fn dismiss_menu(&mut self, app: &mut App, commit: bool) {
        let theme_menu = self.color_theme_menu_open();
        if let Some(panel) = self.panel.as_mut() {
            panel.menu = None;
        }
        if !theme_menu {
            return;
        }
        if commit {
            if let Some(panel) = self.panel.as_mut() {
                panel.theme_before = None;
            }
            return;
        }
        let saved = self
            .panel
            .as_mut()
            .and_then(|panel| panel.theme_before.take());
        if let Some((name, follows)) = saved {
            app.chrome.theme_follows_system = follows;
            let schemes = self.schemes.clone();
            crate::theme_picker::with_schemes(app, &schemes, |app| app.set_theme(&name));
        }
    }

    fn preview_menu_theme(&mut self, app: &mut App) {
        let choice = {
            let Some(panel) = self.panel.as_ref() else {
                return;
            };
            let Some(menu) = panel.menu else {
                return;
            };
            let Some(item) = panel.items.get(menu.item) else {
                return;
            };
            if item.key != "workbench.colorTheme" {
                return;
            }
            let SettingKind::Enum { options, .. } = &item.kind else {
                return;
            };
            options.get(menu.highlight).copied().map(str::to_string)
        };
        let Some(choice) = choice else {
            return;
        };
        if let Some(panel) = self.panel.as_mut()
            && panel.theme_before.is_none()
        {
            panel.theme_before = Some((
                app.chrome.theme.name.to_string(),
                app.chrome.theme_follows_system,
            ));
        }
        let schemes = self.schemes.clone();
        crate::theme_picker::with_schemes(app, &schemes, |app| {
            if choice == "system" {
                let scheme = app.chrome.system_scheme;
                app.set_theme(lux_ui::Chrome::theme_name_for_scheme(scheme));
            } else {
                app.preview_theme(&choice);
            }
        });
    }

    fn edit(&mut self, app: &mut App, index: usize, edit: Edit) {
        let Some((key, new_json)) = self.prepare_edit(index, edit) else {
            return;
        };
        self.update_setting_value(app, key, new_json);
    }

    fn prepare_edit(&mut self, index: usize, edit: Edit) -> Option<(&'static str, Value)> {
        let panel = self.panel.as_mut()?;
        let item = panel.items.get_mut(index)?;
        let new_json = match (&mut item.kind, edit) {
            (SettingKind::Boolean { value }, Edit::SetBool(next)) => {
                if *value == next {
                    return None;
                }
                *value = next;
                Value::Bool(next)
            }
            (SettingKind::Boolean { value }, Edit::Step(forward)) => {
                if *value == forward {
                    return None;
                }
                *value = forward;
                Value::Bool(forward)
            }
            (SettingKind::Number { value, options }, Edit::Step(forward)) => {
                let current = options
                    .iter()
                    .position(|option| (option - *value).abs() < 0.01)
                    .unwrap_or(0);
                let next = if forward {
                    (current + 1).min(options.len().saturating_sub(1))
                } else {
                    current.saturating_sub(1)
                };
                if (options[next] - *value).abs() < 0.01 {
                    return None;
                }
                *value = options[next];
                serde_json::json!(*value)
            }
            (SettingKind::Enum { value, options }, Edit::Step(forward)) => {
                let current = options
                    .iter()
                    .position(|option| *option == value)
                    .unwrap_or(0);
                let next = if forward {
                    (current + 1).min(options.len().saturating_sub(1))
                } else {
                    current.saturating_sub(1)
                };
                if options[next] == value {
                    return None;
                }
                *value = options[next].to_string();
                Value::String(value.clone())
            }
            (SettingKind::Enum { value, options }, Edit::Choose(choice)) => {
                let choice = options.get(choice).copied()?;
                if value == choice {
                    return None;
                }
                *value = choice.to_string();
                Value::String(value.clone())
            }
            _ => return None,
        };
        Some((item.key, new_json))
    }
}

fn fs_read_map(path: &std::path::Path) -> Option<serde_json::Map<String, Value>> {
    let text = std::fs::read_to_string(path).ok()?;
    lux_config::layering::jsonc::parse(&text)
        .ok()
        .and_then(|value| value.as_object().cloned())
}

pub(crate) fn bounds_for(app: &App) -> SettingsBounds {
    let tab_bar = app.tab_bar_height();
    let status_bar = app.status_bar_height();
    if let Some(gpu) = &app.gpu_window {
        let size = gpu.window.inner_size();
        return SettingsBounds {
            window_width: size.width as f32,
            window_height: size.height as f32,
            tab_bar,
            status_bar,
            scale: app.scale_factor,
        };
    }
    SettingsBounds {
        window_width: app.viewport.width + app.sidebar_width() + app.minimap_width(),
        window_height: app.viewport.height
            + tab_bar
            + status_bar
            + app.search_bar_height()
            + app.term_dock_height(),
        tab_bar,
        status_bar,
        scale: app.scale_factor,
    }
}

pub(crate) fn scroll_settings(app: &mut App, delta: ScrollDelta) -> bool {
    if app.ui.settings.panel.is_none() {
        return false;
    }
    cancel_settings_menu(app);
    let scale = app.scale_factor;
    let mut settings = std::mem::take(&mut app.ui.settings);
    let scrolled = settings.scroll_by(delta, scale);
    if scrolled {
        let bounds = bounds_for(app);
        if let Some(panel) = settings.panel.as_mut() {
            let layout = layout_settings(panel, bounds);
            panel.scroll = layout.scroll;
        }
        app.scheduler.invalidate_all();
    }
    app.ui.settings = settings;
    scrolled
}

/// Escape and a scroll close an open settings menu. A color-theme preview
/// returns to the theme that was active when the menu opened.
pub(crate) fn cancel_settings_menu(app: &mut App) {
    let saved = {
        let Some(panel) = app.ui.settings.panel.as_mut() else {
            return;
        };
        let theme_menu = panel.menu.is_some_and(|menu| {
            panel
                .items
                .get(menu.item)
                .is_some_and(|item| item.key == "workbench.colorTheme")
        });
        panel.menu = None;
        if theme_menu {
            panel.theme_before.take()
        } else {
            None
        }
    };
    if let Some((name, follows)) = saved {
        app.chrome.theme_follows_system = follows;
        app.set_theme(&name);
    }
}

fn reveal(panel: &mut SettingsPanelState, bounds: SettingsBounds) {
    let layout = layout_settings(panel, bounds);
    let Some(row) = layout.rows.get(panel.selected_index) else {
        return;
    };
    if row.row.y < layout.body.y {
        panel.scroll -= layout.body.y - row.row.y;
    } else if row.row.bottom() > layout.body.bottom() {
        panel.scroll += row.row.bottom() - layout.body.bottom();
    }
    let layout = layout_settings(panel, bounds);
    panel.scroll = layout.scroll;
}
