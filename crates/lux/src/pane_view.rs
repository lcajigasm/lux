//! Multi-pane editor view state (ST-SPLIT-01).

use lux_core::bookmarks::BookmarkState;
use lux_core::folding::FoldState;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_ui::editor_view::Viewport;
use lux_ui::tabs::TabId;

/// Local viewport bounded to the geometry of a split pane.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct PaneViewport {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) scroll_x: f32,
    pub(crate) scroll_y: f32,
}

impl PaneViewport {
    #[allow(dead_code)]
    pub(crate) fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            scroll_x: 0.0,
            scroll_y: 0.0,
        }
    }

    pub(crate) fn to_editor_viewport(self) -> Viewport {
        Viewport {
            width: self.width,
            height: self.height,
            scroll_x: self.scroll_x,
            scroll_y: self.scroll_y,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn rect(self) -> [f32; 4] {
        [self.x, self.y, self.width, self.height]
    }
}

/// Independent editor view instance per split pane.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PaneView {
    pub(crate) active_tab: TabId,
    pub(crate) tab_id: TabId,
    pub(crate) viewport: PaneViewport,
    pub(crate) cursors: MultiCursor,
    pub(crate) fold_state: FoldState,
    pub(crate) bookmarks: BookmarkState,
    pub(crate) tokens: PaneTokens,
    /// The group's own tabs, in its strip's order. `tab_id` is one of
    /// them; a tab shown in two groups (a cloned view) is in both.
    pub(crate) tabs: Vec<TabId>,
    /// Horizontal scroll of the group's strip, in physical pixels.
    pub(crate) strip_scroll: f32,
    /// The shown tab the strip last scrolled into view.
    pub(crate) revealed: Option<TabId>,
}

impl PaneView {
    pub(crate) fn new(tab_id: TabId) -> Self {
        Self {
            active_tab: tab_id,
            tab_id,
            viewport: PaneViewport::default(),
            cursors: MultiCursor::new(Selection::cursor(Position::document_start())),
            fold_state: FoldState::new(),
            bookmarks: BookmarkState::new(),
            tokens: PaneTokens::default(),
            tabs: vec![tab_id],
            strip_scroll: 0.0,
            revealed: None,
        }
    }

    pub(crate) fn with_geometry(tab_id: TabId, rect: [f32; 4]) -> Self {
        let mut pane = Self::new(tab_id);
        pane.viewport.x = rect[0];
        pane.viewport.y = rect[1];
        pane.viewport.width = rect[2];
        pane.viewport.height = rect[3];
        pane
    }
}

/// A pane's cached highlight tokens, with a generation that changes every
/// time they are replaced. Caches keyed on them (the minimap's colors)
/// compare generations, never addresses: a freed and reallocated vector
/// can come back at the same address with the same length.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct PaneTokens {
    tokens: Vec<lux_syntax::Token>,
    generation: u64,
}

impl PaneTokens {
    pub(crate) fn replace(&mut self, tokens: Vec<lux_syntax::Token>) {
        self.tokens = tokens;
        self.generation = next_generation();
    }

    pub(crate) fn clear(&mut self) {
        self.replace(Vec::new());
    }

    pub(crate) fn as_slice(&self) -> &[lux_syntax::Token] {
        &self.tokens
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Unique across every pane's replacements; 0 only before the first.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
}

fn next_generation() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::PaneTokens;

    #[test]
    fn every_replacement_gets_a_new_generation() {
        let mut tokens = PaneTokens::default();
        let mut seen = vec![tokens.generation()];
        for _ in 0..3 {
            tokens.replace(Vec::new());
            assert!(!seen.contains(&tokens.generation()));
            seen.push(tokens.generation());
        }
        tokens.clear();
        assert!(!seen.contains(&tokens.generation()));
    }
}
