//! Language Mode Picker modal dialog (T02).

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext, ScrollDelta};
use lux_syntax::LanguageId;

pub(crate) struct LanguagePickerState {
    pub(crate) query: crate::field::QueryField,
    pub(crate) selected_index: usize,
    pub(crate) candidates: Vec<(Option<LanguageId>, &'static str)>,
}

impl Default for LanguagePickerState {
    fn default() -> Self {
        Self::new()
    }
}

impl LanguagePickerState {
    pub(crate) fn new() -> Self {
        Self {
            query: crate::field::QueryField::new(),
            selected_index: 0,
            candidates: all_languages(),
        }
    }

    pub(crate) fn filtered(&self) -> Vec<&(Option<LanguageId>, &'static str)> {
        let q = self.query.trim();
        if q.is_empty() {
            return self.candidates.iter().collect();
        }
        lux_search::fuzzy::rank_by(q, &self.candidates, |item| item.1)
            .into_iter()
            .map(|(item, _)| item)
            .collect()
    }
}

/// Routes wheel/trackpad movement to the open language list instead of the
/// editor underneath it. A line moves one row. Trackpad pixels move one row
/// per 16 logical pixels, through the shared overlay.
pub(crate) fn handle_language_picker_scroll(app: &mut crate::app::App, delta: ScrollDelta) -> bool {
    let Some(picker) = app.ui.language_picker.as_mut() else {
        return false;
    };

    let total = picker.filtered().len();
    let rows = crate::overlay_list::wheel_steps(8, delta, app.scale_factor);

    if total > 0 && rows != 0 {
        picker.selected_index = picker
            .selected_index
            .saturating_add_signed(rows)
            .min(total - 1);
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
    }
    true
}

/// Command-line open skips session restore while the file is still loading.
/// The language chosen for that path still has to come back.
pub(crate) fn apply_remembered_language(app: &mut crate::app::App) {
    let Some(path) = app.documents.path.clone() else {
        return;
    };
    let root = app
        .ui
        .sidebar
        .as_ref()
        .and_then(|sidebar| sidebar.workspace_root.as_deref());
    let session_path = app
        .ui
        .session
        .session_path
        .clone()
        .unwrap_or_else(|| crate::session::default_session_path(root));
    let Ok(session) = lux_workspace::session::Session::load(&session_path) else {
        return;
    };
    let Some(label) = session.tabs.iter().rev().find_map(|tab| {
        if tab.path.as_deref() == Some(path.as_path()) {
            tab.language.as_deref()
        } else {
            None
        }
    }) else {
        return;
    };
    if let Some(language) = id_for_label(label) {
        app.set_language_mode(language);
    }
}

pub(crate) fn id_for_label(label: &str) -> Option<Option<LanguageId>> {
    if let Some((id, _)) = all_languages().into_iter().find(|(_, name)| *name == label) {
        return Some(id);
    }
    LanguageId::ALL
        .iter()
        .find(|id| id.name() == label)
        .map(|id| Some(*id))
}

pub(crate) fn all_languages() -> Vec<(Option<LanguageId>, &'static str)> {
    let mut languages = vec![(None, "Plain Text")];
    for language in LanguageId::ALL {
        let name = if *language == LanguageId::Bash {
            "Shell Script (Bash)"
        } else {
            language.name()
        };
        languages.push((Some(*language), name));
    }
    languages
}

pub(crate) fn handle_language_picker_key(
    app: &mut crate::app::App,
    _ctx: &dyn PlatformContext,
    event: &KeyEvent,
) -> bool {
    let total = {
        let Some(picker) = app.ui.language_picker.as_ref() else {
            return false;
        };
        picker.filtered().len()
    };
    let metrics = crate::modal::ModalMetrics::from_app(app);
    let visible = crate::overlay_list::visible_rows(
        metrics.vp_height,
        metrics.tab_bar_height,
        metrics.status_bar,
        metrics.scale,
        total,
    );

    match &event.key {
        Key::Named(NamedKey::Escape) => {
            app.ui.language_picker = None;
            crate::focus::focus_editor(app);
            app.scheduler.invalidate_all();
            true
        }
        key if {
            let Some(picker) = app.ui.language_picker.as_mut() else {
                return false;
            };
            crate::overlay_list::nudge(&mut picker.selected_index, total, visible, key)
        } =>
        {
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Backspace) => {
            if let Some(picker) = app.ui.language_picker.as_mut() {
                picker.query.pop();
                picker.selected_index = 0;
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Enter) => {
            let selected_lang = {
                let Some(picker) = app.ui.language_picker.as_ref() else {
                    return false;
                };
                let filtered = picker.filtered();
                filtered.get(picker.selected_index).map(|(lang, _)| *lang)
            };
            app.ui.language_picker = None;
            crate::focus::focus_editor(app);
            if let Some(lang) = selected_lang {
                app.set_language_mode(lang);
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Character(s) if !app.modifiers.meta && !app.modifiers.control => {
            if let Some(picker) = app.ui.language_picker.as_mut() {
                let text = event.text.as_deref().unwrap_or(s);
                if !text.chars().any(char::is_control) {
                    picker.query.push_str(text);
                }
                picker.selected_index = 0;
            }
            app.scheduler.invalidate_all();
            true
        }
        _ => false,
    }
}

fn picker_card(
    app: &crate::app::App,
    selected: usize,
    total: usize,
) -> crate::overlay_list::ListFrame {
    let metrics = crate::modal::ModalMetrics::from_app(app);
    crate::overlay_list::centered_frame(
        metrics.vp_width,
        metrics.vp_height,
        metrics.tab_bar_height,
        metrics.status_bar,
        metrics.scale,
        total,
        selected,
    )
}

pub(crate) fn handle_language_picker_click(
    app: &mut crate::app::App,
    mouse_x: f32,
    mouse_y: f32,
) -> bool {
    let choice = {
        let Some(picker) = app.ui.language_picker.as_ref() else {
            return false;
        };
        let filtered = picker.filtered();
        let card = picker_card(app, picker.selected_index, filtered.len());
        let inside = mouse_x >= card.left
            && mouse_x <= card.left + card.width
            && mouse_y >= card.top
            && mouse_y <= card.top + card.height;
        if !inside {
            None
        } else if mouse_y >= card.rows_top && card.row_height > 0.0 {
            let display_idx = ((mouse_y - card.rows_top) / card.row_height) as usize;
            let actual_idx = card.origin + display_idx;
            if display_idx < card.visible && actual_idx < filtered.len() {
                Some(Some(filtered[actual_idx].0))
            } else {
                Some(None)
            }
        } else {
            Some(None)
        }
    };
    match choice {
        None => {
            app.ui.language_picker = None;
            crate::focus::focus_editor(app);
            app.scheduler.invalidate_all();
        }
        Some(Some(lang)) => {
            app.ui.language_picker = None;
            crate::focus::focus_editor(app);
            app.set_language_mode(lang);
            app.scheduler.invalidate_all();
        }
        Some(None) => {}
    }
    true
}

pub(crate) fn handle_language_picker_hover(
    app: &mut crate::app::App,
    mouse_x: f32,
    mouse_y: f32,
) -> bool {
    let (total, selected) = {
        let Some(picker) = app.ui.language_picker.as_ref() else {
            return false;
        };
        (picker.filtered().len(), picker.selected_index)
    };
    let card = picker_card(app, selected, total);
    let rows_bottom = card.rows_top + card.visible as f32 * card.row_height;
    if mouse_x < card.left
        || mouse_x > card.left + card.width
        || mouse_y < card.rows_top
        || mouse_y >= rows_bottom
        || card.row_height <= 0.0
    {
        return false;
    }
    let actual_idx = card.origin + ((mouse_y - card.rows_top) / card.row_height) as usize;
    if actual_idx < total && actual_idx != selected {
        if let Some(picker) = app.ui.language_picker.as_mut() {
            picker.selected_index = actual_idx;
        }
        app.scheduler.invalidate_all();
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use lux_core::buffer::Buffer;

    #[test]
    fn language_picker_initializes_with_all_supported_languages() {
        let picker = LanguagePickerState::new();
        assert_eq!(picker.query, "");
        assert_eq!(picker.selected_index, 0);
        assert!(picker.candidates.len() >= 19);
        assert!(
            picker
                .candidates
                .iter()
                .any(|(id, name)| *name == "Rust" && *id == Some(LanguageId::Rust))
        );
        assert!(
            picker
                .candidates
                .iter()
                .any(|(id, name)| *name == "Plain Text" && id.is_none())
        );
    }

    #[test]
    fn language_picker_filters_by_query() {
        let mut picker = LanguagePickerState::new();
        picker.query = crate::field::QueryField::from_text("python");
        let results = picker.filtered();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "Python");
        assert_eq!(results[0].0, Some(LanguageId::Python));

        picker.query = crate::field::QueryField::from_text("type");
        let type_results = picker.filtered();
        assert_eq!(type_results.len(), 2); // TypeScript and TypeScript (TSX)
    }

    #[test]
    fn shebang_detection_identifies_shell_and_python() {
        let py_buf = Buffer::from_bytes(b"#!/usr/bin/env python3\nprint('hello')\n").unwrap();
        assert_eq!(
            crate::document::detect_language(None, &py_buf),
            Some(LanguageId::Python)
        );

        let bash_buf = Buffer::from_bytes(b"#!/bin/bash\necho 'hello'\n").unwrap();
        assert_eq!(
            crate::document::detect_language(None, &bash_buf),
            Some(LanguageId::Bash)
        );
    }

    #[test]
    fn language_picker_click_selects_language_and_dismisses() {
        let mut app = crate::app::App::new(
            include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec(),
        );
        app.open_language_picker();
        assert!(app.ui.language_picker.is_some());

        // The language list uses the same card as the command palette.
        // Row 1 is Rust. Its midpoint sits on the shared row pitch.
        let card = picker_card(
            &app,
            0,
            app.ui.language_picker.as_ref().unwrap().filtered().len(),
        );
        let y = card.rows_top + card.row_height * 1.5;
        let x = card.left + card.width * 0.5;
        let clicked = handle_language_picker_click(&mut app, x, y);
        assert!(clicked);
        assert!(app.ui.language_picker.is_none());
        assert_eq!(app.documents.language(), Some(LanguageId::Rust));
    }
}
