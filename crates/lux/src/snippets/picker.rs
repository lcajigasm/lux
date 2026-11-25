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
        self.ranked()
            .into_iter()
            .map(|(snippet, _)| snippet)
            .collect()
    }

    /// Matching snippets, best first. The query ranks against
    /// `name trigger description`; the hits are the part of that match in
    /// the name, the title the list shows.
    pub(crate) fn ranked(&self) -> Vec<(&Snippet, crate::overlay_list::Hits)> {
        let q = self.query.trim();
        if q.is_empty() {
            return self.candidates.iter().map(|s| (s, Vec::new())).collect();
        }
        let labels: Vec<(usize, String)> = self
            .candidates
            .iter()
            .enumerate()
            .map(|(index, snippet)| {
                (
                    index,
                    format!(
                        "{} {} {}",
                        snippet.name, snippet.trigger, snippet.description
                    ),
                )
            })
            .collect();
        lux_search::fuzzy::rank_by(q, &labels, |(_, label)| label.as_str())
            .into_iter()
            .map(|ranked| {
                let (index, label) = ranked.item;
                let snippet = &self.candidates[*index];
                let name = 0..snippet.name.len();
                let hits = lux_search::fuzzy::char_spans_in(label, &ranked.indices, name);
                (snippet, hits)
            })
            .collect()
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
            app.ui.snippets.picker = None;
            app.scheduler.invalidate_all();
            true
        }
        key if {
            let Some(picker) = app.ui.snippets.picker.as_mut() else {
                return false;
            };
            crate::overlay_list::nudge(&mut picker.selected_index, total, visible, key)
        } =>
        {
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
        Key::Character(s) if !crate::field::is_shortcut_chord(app) => {
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
