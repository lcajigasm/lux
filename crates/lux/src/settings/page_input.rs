//! Keys and clicks on the settings page.
//!
//! Every handler works on `app` in place. The settings state is never
//! swapped out of `app.ui.settings`, so nothing rebuilds it (its
//! constructor creates the settings file, scans themes and starts a
//! watcher). A changed value shows on the page at once; the file write runs
//! on a worker and the editor applies the value when it lands.

use lux_platform::event::{ElementState, KeyEvent};
use lux_platform::{Key, NamedKey};

use super::layout::{SettingsHit, layout_settings};
use super::panel::{Edit, SettingItem, SettingKind, bounds_for};
use crate::app::App;

pub(crate) fn handle_key(app: &mut App, event: &KeyEvent) -> bool {
    if event.state != ElementState::Pressed || app.ui.settings.panel.is_none() {
        return false;
    }
    let bounds = bounds_for(app);
    let settings = &mut app.ui.settings;
    match &event.key {
        Key::Named(NamedKey::Escape) => {
            match settings.panel.as_mut() {
                Some(panel) if panel.menu.is_some() => panel.menu = None,
                _ => settings.panel = None,
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowDown) => {
            settings.move_selection(bounds, 1);
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowUp) => {
            settings.move_selection(bounds, -1);
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowRight) => {
            nudge_selected(app, true);
            true
        }
        Key::Named(NamedKey::ArrowLeft) => {
            nudge_selected(app, false);
            true
        }
        Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => {
            activate_selected(app);
            true
        }
        Key::Named(NamedKey::Backspace) => {
            edit_query(app, None);
            true
        }
        Key::Named(NamedKey::PageDown) => scroll_page(app, 1.0),
        Key::Named(NamedKey::PageUp) => scroll_page(app, -1.0),
        Key::Named(NamedKey::End) => scroll_page(app, f32::MAX),
        Key::Named(NamedKey::Home) => scroll_page(app, f32::MIN),
        Key::Character(text)
            if !app.modifiers.meta && !app.modifiers.control && !app.modifiers.alt =>
        {
            edit_query(app, Some(text.clone()));
            true
        }
        _ => true,
    }
}

fn edit_query(app: &mut App, insert: Option<String>) {
    let Some(panel) = app.ui.settings.panel.as_mut() else {
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

pub(crate) fn handle_click(app: &mut App, mouse_x: f32, mouse_y: f32) -> bool {
    let bounds = bounds_for(app);
    let Some(panel) = app.ui.settings.panel.as_mut() else {
        return false;
    };
    let hit = layout_settings(panel, bounds).hit(mouse_x, mouse_y);
    match hit {
        SettingsHit::Outside | SettingsHit::Close => {
            app.ui.settings.panel = None;
            crate::focus::focus_editor(app);
        }
        SettingsHit::Page | SettingsHit::Search => panel.menu = None,
        SettingsHit::Row(index) => {
            panel.selected_index = index;
            panel.menu = None;
        }
        SettingsHit::Toggle(index) => {
            let on = matches!(
                panel.items.get(index).map(|item| &item.kind),
                Some(SettingKind::Boolean { value: true })
            );
            app.ui.settings.select(index);
            edit(app, index, Edit::SetBool(!on));
            return true;
        }
        SettingsHit::StepMinus(index) | SettingsHit::StepPlus(index) => {
            app.ui.settings.select(index);
            let forward = matches!(hit, SettingsHit::StepPlus(_));
            edit(app, index, Edit::Step(forward));
            return true;
        }
        SettingsHit::MenuButton(index) => {
            app.ui.settings.toggle_menu(index, bounds);
            return true;
        }
        SettingsHit::MenuOption { item, option } => {
            edit(app, item, Edit::Choose(option));
            if let Some(panel) = app.ui.settings.panel.as_mut() {
                panel.menu = None;
                panel.selected_index = item;
            }
            return true;
        }
        SettingsHit::Category(index) => {
            panel.category = index;
            panel.scroll = 0.0;
            panel.menu = None;
            panel.query.clear();
            if let Some(first) = panel.visible_indices().first().copied() {
                panel.selected_index = first;
            }
        }
    }
    app.scheduler.invalidate_all();
    true
}

fn nudge_selected(app: &mut App, forward: bool) {
    let Some(panel) = app.ui.settings.panel.as_mut() else {
        return;
    };
    let index = panel.selected_index;
    panel.menu = None;
    let kind = match panel.items.get(index) {
        Some(SettingItem {
            kind: SettingKind::Boolean { .. },
            ..
        }) => Edit::SetBool(forward),
        _ => Edit::Step(forward),
    };
    edit(app, index, kind);
}

fn activate_selected(app: &mut App) {
    let bounds = bounds_for(app);
    let Some(panel) = app.ui.settings.panel.as_mut() else {
        return;
    };
    if let Some(menu) = panel.menu {
        edit(app, menu.item, Edit::Choose(menu.highlight));
        if let Some(panel) = app.ui.settings.panel.as_mut() {
            panel.menu = None;
        }
        return;
    }
    let index = panel.selected_index;
    match panel.items.get(index).map(|item| &item.kind) {
        Some(SettingKind::Boolean { value }) => {
            let next = !*value;
            edit(app, index, Edit::SetBool(next));
        }
        Some(SettingKind::Enum { .. }) => app.ui.settings.toggle_menu(index, bounds),
        Some(SettingKind::Number { .. }) => edit(app, index, Edit::Step(true)),
        Some(SettingKind::Info) | None => {}
    }
}

fn scroll_page(app: &mut App, pages: f32) -> bool {
    let bounds = bounds_for(app);
    let Some(panel) = app.ui.settings.panel.as_mut() else {
        return true;
    };
    let body = layout_settings(panel, bounds).body.h;
    panel.scroll = if pages >= f32::MAX / 2.0 {
        f32::MAX
    } else if pages <= f32::MIN / 2.0 {
        0.0
    } else {
        (panel.scroll + pages * body).max(0.0)
    };
    panel.menu = None;
    panel.scroll = layout_settings(panel, bounds).scroll;
    app.scheduler.invalidate_all();
    true
}

/// Shows the new value on the page and queues the file write.
fn edit(app: &mut App, index: usize, edit: Edit) {
    let Some((key, value)) = app.ui.settings.prepare_edit(index, edit) else {
        return;
    };
    super::set_setting(app, key, value);
    app.scheduler.invalidate_all();
}
