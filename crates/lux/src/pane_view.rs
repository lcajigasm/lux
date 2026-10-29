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
    pub(crate) tokens: Vec<lux_syntax::Token>,
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
            tokens: Vec::new(),
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
