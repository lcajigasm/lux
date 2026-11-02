//! Language Mode Picker modal dialog (T02).

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext, ScrollDelta};
use lux_syntax::LanguageId;

pub(crate) struct LanguagePickerState {
    pub(crate) query: String,
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
            query: String::new(),
            selected_index: 0,
            candidates: all_languages(),
            scroll_remainder: 0.0,
        }
    }

    pub(crate) fn filtered(&self) -> Vec<&(Option<LanguageId>, &'static str)> {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() {
            self.candidates.iter().collect()
        } else {
            self.candidates
                .iter()
                .filter(|(_, name)| name.to_lowercase().contains(&q))
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
            let row_height = 26.0 * app.scale_factor.max(1.0);
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
    vec![
        (None, "Plain Text"),
        (Some(LanguageId::Rust), "Rust"),
        (Some(LanguageId::TypeScript), "TypeScript"),
        (Some(LanguageId::Tsx), "TypeScript (TSX)"),
        (Some(LanguageId::JavaScript), "JavaScript"),
        (Some(LanguageId::Python), "Python"),
        (Some(LanguageId::Go), "Go"),
        (Some(LanguageId::C), "C"),
        (Some(LanguageId::Cpp), "C++"),
        (Some(LanguageId::Java), "Java"),
        (Some(LanguageId::Kotlin), "Kotlin"),
        (Some(LanguageId::Swift), "Swift"),
        (Some(LanguageId::Html), "HTML"),
        (Some(LanguageId::Css), "CSS"),
        (Some(LanguageId::Json), "JSON"),
        (Some(LanguageId::Yaml), "YAML"),
        (Some(LanguageId::Markdown), "Markdown"),
        (Some(LanguageId::Bash), "Shell Script (Bash)"),
        (Some(LanguageId::Toml), "TOML"),
    ]
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

pub(crate) fn handle_language_picker_click(
    app: &mut crate::app::App,
    mouse_x: f32,
    mouse_y: f32,
) -> bool {
    let Some(picker) = app.ui.language_picker.as_ref() else {
        return false;
    };

    let window_width = app
        .gpu_window
        .as_ref()
        .map(|g| g.window.inner_size().width as f32)
        .unwrap_or(app.viewport.width);
    let tab_bar_height = app.tab_bar_height();
    let scale_factor = app.scale_factor;
    let stroke = scale_factor;

    let width = (window_width * 0.52).clamp(340.0 * scale_factor, 560.0 * scale_factor);
    let left = (window_width - width) * 0.5;
    let top = tab_bar_height + 12.0 * scale_factor;
    let input_height = 34.0 * scale_factor;
    let row_height = 26.0 * scale_factor;

    let filtered = picker.filtered();
    let visible_count = filtered.len().min(9);
    let total_height = input_height + (visible_count as f32 * row_height) + 4.0 * stroke;

    let in_bounds = mouse_x >= left
        && mouse_x <= left + width
        && mouse_y >= top
        && mouse_y <= top + total_height;

    if !in_bounds {
        app.ui.language_picker = None;
        app.scheduler.invalidate_all();
        return true;
    }

    let rows_top = top + input_height + 2.0 * stroke;
    if mouse_y >= rows_top {
        let display_idx = ((mouse_y - rows_top) / row_height) as usize;
        let start_idx = if picker.selected_index >= visible_count {
            picker.selected_index + 1 - visible_count
        } else {
            0
        };
        let actual_idx = start_idx + display_idx;
        if actual_idx < filtered.len() {
            let (chosen_lang, _) = filtered[actual_idx];
            let lang = *chosen_lang;
            app.ui.language_picker = None;
            app.set_language_mode(lang);
            app.scheduler.invalidate_all();
            return true;
        }
    }

    true
}

pub(crate) fn handle_language_picker_hover(
    app: &mut crate::app::App,
    mouse_x: f32,
    mouse_y: f32,
) -> bool {
    let window_width = app
        .gpu_window
        .as_ref()
        .map(|g| g.window.inner_size().width as f32)
        .unwrap_or(app.viewport.width);
    let tab_bar_height = app.tab_bar_height();
    let scale_factor = app.scale_factor;
    let stroke = scale_factor;

    let Some(picker) = app.ui.language_picker.as_mut() else {
        return false;
    };

    let width = (window_width * 0.52).clamp(340.0 * scale_factor, 560.0 * scale_factor);
    let left = (window_width - width) * 0.5;
    let top = tab_bar_height + 12.0 * scale_factor;
    let input_height = 34.0 * scale_factor;
    let row_height = 26.0 * scale_factor;

    let filtered = picker.filtered();
    let visible_count = filtered.len().min(9);
    let rows_top = top + input_height + 2.0 * stroke;
    let rows_bottom = rows_top + (visible_count as f32 * row_height);

    if mouse_x >= left && mouse_x <= left + width && mouse_y >= rows_top && mouse_y <= rows_bottom {
        let display_idx = ((mouse_y - rows_top) / row_height) as usize;
        let start_idx = if picker.selected_index >= visible_count {
            picker.selected_index + 1 - visible_count
        } else {
            0
        };
        let actual_idx = start_idx + display_idx;
        if actual_idx < filtered.len() && actual_idx != picker.selected_index {
            picker.selected_index = actual_idx;
            app.scheduler.invalidate_all();
            return true;
        }
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
        picker.query = "python".to_string();
        let results = picker.filtered();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "Python");
        assert_eq!(results[0].0, Some(LanguageId::Python));

        picker.query = "type".to_string();
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

        // Modal top is 44px (tab_bar_height=32, offset=12).
        // Input height is 34. Row 0 starts at 44 + 34 + 2 = 80. Row height is 26.
        // Row 1 (Rust) is at 106..132 (midpoint 119.0).
        // Horizontal center is 500.0 (viewport width 1000).
        let clicked = handle_language_picker_click(&mut app, 500.0, 119.0);
        assert!(clicked);
        assert!(app.ui.language_picker.is_none());
        assert_eq!(app.documents.language(), Some(LanguageId::Rust));
    }
}
