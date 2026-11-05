//! Language Mode Picker modal dialog (T02).

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext, ScrollDelta};
use lux_syntax::LanguageId;

pub(crate) struct LanguagePickerState {
    pub(crate) query: crate::field::QueryField,
    pub(crate) selected_index: usize,
    pub(crate) candidates: Vec<(Option<LanguageId>, &'static str)>,
    scroll_remainder: f32,
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
            scroll_remainder: 0.0,
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

/// Routes wheel/trackpad movement to the open language list instead of the
/// editor underneath it. Line deltas move discrete rows; precise pixel
/// deltas accumulate so a trackpad does not skip rows or lose small motions.
pub(crate) fn handle_language_picker_scroll(app: &mut crate::app::App, delta: ScrollDelta) -> bool {
    let Some(picker) = app.ui.language_picker.as_mut() else {
        return false;
    };

    let total = picker.filtered().len();
    let rows = match delta {
        ScrollDelta::Lines { y, .. } => (-y).round() as isize,
        ScrollDelta::Pixels(position) => {
            let row_height = crate::render::panel::selector_box(1.0, 0.0, 1.0).row_height
                * app.scale_factor.max(1.0);
            picker.scroll_remainder += -(position.y as f32);
            let rows = (picker.scroll_remainder / row_height).trunc() as isize;
            picker.scroll_remainder -= rows as f32 * row_height;
            rows
        }
    };

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

    match &event.key {
        Key::Named(NamedKey::Escape) => {
            app.ui.language_picker = None;
            crate::focus::focus_editor(app);
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowDown) => {
            if let Some(picker) = app.ui.language_picker.as_mut() {
                picker.select_next(total);
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowUp) => {
            if let Some(picker) = app.ui.language_picker.as_mut() {
                picker.select_prev();
            }
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

struct PickerCard {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    rows_top: f32,
    row_height: f32,
    visible: usize,
    start: usize,
}

fn picker_card(app: &crate::app::App, selected: usize, total: usize) -> PickerCard {
    use crate::render::panel::{self, SELECTOR_VISIBLE};
    let width = app
        .gpu_window
        .as_ref()
        .map(|g| g.window.inner_size().width as f32)
        .unwrap_or(app.viewport.width);
    let scale = app.scale_factor;
    let box_ = panel::selector_box(width, app.tab_bar_height(), scale);
    let visible = total.min(SELECTOR_VISIBLE);
    let start = if selected >= visible {
        selected + 1 - visible
    } else {
        0
    };
    PickerCard {
        left: box_.left,
        top: box_.top,
        width: box_.width,
        height: panel::panel_height(box_.input_height, box_.row_height, visible, scale),
        rows_top: panel::rows_top(box_.top, box_.input_height, scale),
        row_height: box_.row_height,
        visible,
        start,
    }
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
            let actual_idx = card.start + display_idx;
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
    let actual_idx = card.start + ((mouse_y - card.rows_top) / card.row_height) as usize;
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
