//! Language Mode Picker modal dialog (T02).

use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext};
use lux_syntax::LanguageId;

pub(crate) struct LanguagePickerState {
    pub(crate) query: String,
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
            query: String::new(),
            selected_index: 0,
            candidates: all_languages(),
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
        Key::Character(s) => {
            if let Some(picker) = app.ui.language_picker.as_mut() {
                picker.query.push_str(s);
                picker.selected_index = 0;
            }
            app.scheduler.invalidate_all();
            true
        }
        _ => false,
    }
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
}
