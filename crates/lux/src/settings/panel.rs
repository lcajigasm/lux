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
    pub(crate) category: &'static str,
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
    pub(crate) query: String,
    /// Index into [`SettingsPanelState::categories`]. 0 is All.
    pub(crate) category: usize,
}

impl SettingsPanelState {
    pub(crate) fn categories(&self) -> Vec<&'static str> {
        let mut out = vec!["All"];
        for item in &self.items {
            if !out.contains(&item.category) {
                out.push(item.category);
            }
        }
        out
    }

    pub(crate) fn visible_indices(&self) -> Vec<usize> {
        let categories = self.categories();
        let selected = categories.get(self.category).copied().unwrap_or("All");
        let query = self.query.trim().to_ascii_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                if query.is_empty() {
                    return selected == "All" || item.category == selected;
                }
                item.label.to_ascii_lowercase().contains(&query)
                    || item.description.to_ascii_lowercase().contains(&query)
                    || item.key.contains(query.as_str())
            })
            .map(|(index, _)| index)
            .collect()
    }
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
            query: String::new(),
            category: 0,
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
                    if let Some(panel) = self.panel.as_mut() {
                        panel.menu = None;
                    }
                } else {
                    self.panel = None;
                }
                app.scheduler.invalidate_all();
                true
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.move_selection(bounds, 1);
                app.scheduler.invalidate_all();
                true
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.move_selection(bounds, -1);
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
            Key::Named(NamedKey::Backspace) => {
                self.edit_query(app, None);
                true
            }
            Key::Character(text)
                if !app.modifiers.meta && !app.modifiers.control && !app.modifiers.alt =>
            {
                self.edit_query(app, Some(text.clone()));
                true
            }
            _ => true,
        }
    }

    fn edit_query(&mut self, app: &mut App, insert: Option<String>) {
        let Some(panel) = self.panel.as_mut() else {
            return;
        };
        match insert {
            Some(text) => panel.query.push_str(&text),
            None => {
                panel.query.pop();
            }
        }
        panel.scroll = 0.0;
        panel.menu = None;
        if let Some(first) = panel.visible_indices().first().copied() {
            panel.selected_index = first;
        }
        app.scheduler.invalidate_all();
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
                if let Some(panel) = self.panel.as_mut() {
                    panel.menu = None;
                }
                app.scheduler.invalidate_all();
            }
            SettingsHit::Row(index) => {
                if let Some(panel) = self.panel.as_mut() {
                    panel.selected_index = index;
                    panel.menu = None;
                }
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
            SettingsHit::MenuButton(index) => self.toggle_menu(index),
            SettingsHit::MenuOption { item, option } => {
                self.edit(app, item, Edit::Choose(option));
                if let Some(panel) = self.panel.as_mut() {
                    panel.menu = None;
                    panel.selected_index = item;
                }
            }
            SettingsHit::Category(index) => {
                if let Some(panel) = self.panel.as_mut() {
                    panel.category = index;
                    panel.scroll = 0.0;
                    panel.menu = None;
                    panel.query.clear();
                    if let Some(first) = panel.visible_indices().first().copied() {
                        panel.selected_index = first;
                    }
                }
                app.scheduler.invalidate_all();
            }
            SettingsHit::Search => {
                if let Some(panel) = self.panel.as_mut() {
                    panel.menu = None;
                }
                app.scheduler.invalidate_all();
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
        panel.menu = None;
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
        let visible = panel.visible_indices();
        if visible.is_empty() {
            return;
        }
        let pos = visible
            .iter()
            .position(|index| *index == panel.selected_index)
            .unwrap_or(0);
        let next = (pos as isize + delta).clamp(0, visible.len() as isize - 1) as usize;
        panel.selected_index = visible[next];
        reveal(panel, bounds);
    }

    fn nudge_selected(&mut self, app: &mut App, forward: bool) {
        let Some(index) = self.panel.as_ref().map(|panel| panel.selected_index) else {
            return;
        };
        if let Some(panel) = self.panel.as_mut() {
            panel.menu = None;
        }
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
            if let Some(panel) = self.panel.as_mut() {
                panel.menu = None;
            }
            return;
        }
        let index = panel.selected_index;
        match panel.items.get(index).map(|item| &item.kind) {
            Some(SettingKind::Boolean { value }) => {
                let next = !*value;
                self.edit(app, index, Edit::SetBool(next));
            }
            Some(SettingKind::Enum { .. }) => self.toggle_menu(index),
            Some(SettingKind::Number { .. }) => self.edit(app, index, Edit::Step(true)),
            None => {}
        }
    }

    fn toggle_menu(&mut self, index: usize) {
        let Some(panel) = self.panel.as_mut() else {
            return;
        };
        panel.selected_index = index;
        let open = panel.menu.is_some_and(|menu| menu.item == index);
        if open {
            panel.menu = None;
            return;
        }
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
    }

    fn select(&mut self, index: usize) {
        if let Some(panel) = self.panel.as_mut() {
            panel.selected_index = index;
            panel.menu = None;
        }
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
        // A scale snapshot sets the viewport to the capture, which is
        // larger than the live window. Lay the page out in that frame
        // so the sidebar, search field, and scrollbar land in the PNG.
        let (window_width, window_height) = if app.viewport.width > size.width as f32 + 1.0 {
            (
                app.viewport.width,
                app.viewport.height + tab_bar + status_bar,
            )
        } else {
            (size.width as f32, size.height as f32)
        };
        return SettingsBounds {
            window_width,
            window_height,
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

fn reveal(panel: &mut SettingsPanelState, bounds: SettingsBounds) {
    let layout = layout_settings(panel, bounds);
    let Some(row) = layout
        .rows
        .iter()
        .find(|row| row.index == panel.selected_index)
    else {
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
