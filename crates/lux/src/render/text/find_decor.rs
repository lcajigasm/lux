//! Colors of the `Find Results` tab: file headers in the accent (moved
//! toward black or white when the accent is too faint on the editor),
//! line-number gutters and the search/summary lines muted, and every hit
//! boxed with the find-bar match fill. The buffer text is not touched;
//! see [`crate::find_results::decor`].

use lux_core::buffer::Buffer;
use lux_core::position::Position;
use lux_ui::editor_view::EditorViewFrame;

use crate::find_results::decor::{LineDecor, Role, decorate, matcher};
use crate::render::paint::color;
use crate::render::panel::readable_ink;

/// Styling of the active buffer when it is the `Find Results` tab.
pub(crate) struct FindDecor {
    matcher: Option<regex::Regex>,
    pub header: [f32; 4],
    pub muted: [f32; 4],
    pub hit: [f32; 4],
}

impl FindDecor {
    pub(crate) fn new(theme: &lux_ui::Theme, matcher: Option<regex::Regex>) -> Self {
        let background = color(theme.background);
        Self {
            matcher,
            header: readable_ink(color(theme.accent), background),
            muted: readable_ink(color(theme.muted), background),
            hit: crate::render::search::match_fill(theme),
        }
    }

    /// Roles and hits of buffer line `index`.
    pub(crate) fn line(&self, buffer: &Buffer, index: usize) -> LineDecor {
        let text = buffer.line_content(index).unwrap_or_default();
        decorate(&text, self.matcher.as_ref())
    }

    /// Glyph ink for `role`; `None` keeps the default text color.
    pub(crate) fn ink(&self, role: Role) -> Option<[f32; 4]> {
        match role {
            Role::Header => Some(self.header),
            Role::Meta | Role::Number => Some(self.muted),
            Role::Text => None,
        }
    }

    /// Document-space boxes of the hits on line `index`, as selections are
    /// measured.
    pub(crate) fn hit_rects(
        &self,
        frame: &EditorViewFrame,
        buffer: &Buffer,
        index: usize,
        decor: &LineDecor,
        line_height: f32,
    ) -> Vec<lux_ui::SelectionRect> {
        let start = buffer.line_start_byte(index).unwrap_or(0);
        decor
            .hits
            .iter()
            .flat_map(|hit| {
                let from = Position::from_byte(buffer, start + hit.start);
                let to = Position::from_byte(buffer, start + hit.end);
                frame.range_rects(buffer, from, to, line_height)
            })
            .collect()
    }
}

/// The decoration for this frame, or `None` when the active tab is not
/// `Find Results`.
pub(crate) fn for_app(app: &mut crate::app::App) -> Option<FindDecor> {
    let active = app.documents.active_tab?;
    let is_results = app
        .documents
        .tabs
        .tabs()
        .iter()
        .any(|tab| tab.id == active && tab.title == crate::find_results::TAB_TITLE);
    if !is_results {
        return None;
    }
    let first = app.buffer.line_content(0).unwrap_or_default();
    let regex = matcher(&mut app.ui.find_results_nav, &first);
    Some(FindDecor::new(&app.chrome.theme, regex))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::panel::contrast_ratio;

    #[test]
    fn headers_and_gutters_are_legible_in_every_bundled_theme() {
        for scheme in crate::color_scheme::bundled_schemes() {
            let theme = &scheme.theme;
            let decor = FindDecor::new(theme, None);
            let background = color(theme.background);
            for (name, ink) in [("header", decor.header), ("muted", decor.muted)] {
                let ratio = contrast_ratio(ink, background);
                assert!(ratio >= 4.5, "{} {name} {ratio:.2}", scheme.name);
            }
        }
    }

    #[test]
    fn decorating_reads_the_buffer_without_changing_it() {
        let text = "Searching files for \"x\"\n\nsrc/a.rs:\n     3: let x = 1;\n\n1 match across 1 file (2 files searched)\n";
        let buffer = Buffer::from_bytes(text.as_bytes()).unwrap();
        let before = buffer.text();
        let decor = FindDecor::new(&lux_ui::Theme::MONOKAI, regex::Regex::new("x").ok());
        let mut hits = 0;
        for index in 0..buffer.len_lines() {
            hits += decor.line(&buffer, index).hits.len();
        }
        assert_eq!(hits, 1);
        assert_eq!(buffer.text(), before);
        assert_eq!(decor.ink(Role::Header), Some(decor.header));
        assert_eq!(decor.ink(Role::Text), None);
    }
}
