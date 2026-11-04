//! Snippet picker modal dialog (`Snippets: Insert Snippet...`).

use super::registry::Snippet;
use lux_platform::event::KeyEvent;
use lux_platform::{Key, NamedKey, PlatformContext};

pub(crate) struct SnippetPickerState {
    pub(crate) query: crate::field::QueryField,
    pub(crate) selected_index: usize,
    pub(crate) candidates: Vec<Snippet>,
}

impl SnippetPickerState {
    pub(crate) fn new(candidates: Vec<Snippet>) -> Self {
        Self {
            query: crate::field::QueryField::new(),
            selected_index: 0,
            candidates,
        }
    }

    pub(crate) fn filtered(&self) -> Vec<&Snippet> {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() {
            self.candidates.iter().collect()
        } else {
            self.candidates
                .iter()
                .filter(|s| {
                    s.trigger.to_lowercase().contains(&q)
                        || s.name.to_lowercase().contains(&q)
                        || s.description.to_lowercase().contains(&q)
                })
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

pub(crate) fn handle_snippet_picker_key(
    app: &mut crate::app::App,
    _ctx: &dyn PlatformContext,
    event: &KeyEvent,
) -> bool {
    let total = {
        let Some(picker) = app.ui.snippets.picker.as_ref() else {
            return false;
        };
        picker.filtered().len()
    };

    match &event.key {
        Key::Named(NamedKey::Escape) => {
            app.ui.snippets.picker = None;
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowDown) => {
            if let Some(picker) = app.ui.snippets.picker.as_mut() {
                picker.select_next(total);
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::ArrowUp) => {
            if let Some(picker) = app.ui.snippets.picker.as_mut() {
                picker.select_prev();
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Backspace) => {
            if let Some(picker) = app.ui.snippets.picker.as_mut() {
                picker.query.pop();
                picker.selected_index = 0;
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Named(NamedKey::Enter) => {
            let template = {
                let Some(picker) = app.ui.snippets.picker.as_ref() else {
                    return false;
                };
                let filtered = picker.filtered();
                filtered
                    .get(picker.selected_index)
                    .map(|s| s.content.clone())
            };
            app.ui.snippets.picker = None;
            if let Some(tmpl) = template {
                app.expand_snippet(&tmpl);
            }
            app.scheduler.invalidate_all();
            true
        }
        Key::Character(s) => {
            if let Some(picker) = app.ui.snippets.picker.as_mut() {
                picker.query.push_str(s);
                picker.selected_index = 0;
            }
            app.scheduler.invalidate_all();
            true
        }
        _ => false,
    }
}
