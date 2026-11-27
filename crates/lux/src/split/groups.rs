//! Where each split group sits in the window (ST-SPLIT-01).
//!
//! Every geometry consumer — the renderer, mouse hit-testing, the pointer
//! shape, the wheel, the minimap — reads the rectangles from here, so a
//! group draws exactly where it takes input. A group owns its editor
//! column, minimap included: the minimap is the rightmost strip of its own
//! group, never a window-wide column over a neighbor's text.

use crate::app::App;
use crate::split::{SplitLayout, compute_layout_geometry};

/// One group's rectangles, `[x, y, width, height]` in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GroupRects {
    /// The whole editor column: text, gutter and minimap.
    pub(crate) editor: [f32; 4],
    /// Gutter and text: the editor column minus the minimap.
    pub(crate) text: [f32; 4],
    /// The group's minimap, when the minimap is on and there is room.
    pub(crate) minimap: Option<[f32; 4]>,
}

/// The rectangle the groups divide: right of the sidebar, below the tab
/// row, through the minimap column to the window's right edge.
pub(crate) fn editor_area(app: &App) -> [f32; 4] {
    [
        app.sidebar_width(),
        app.tab_bar_height(),
        app.viewport.width + app.minimap_width(),
        app.viewport.height,
    ]
}

/// Pure layout: the groups of `split` inside `area`, each with a minimap
/// `minimap_w` wide at its right edge. A group narrower than twice the
/// minimap keeps all of its width for text.
pub(crate) fn layout(
    split: &SplitLayout,
    area: [f32; 4],
    stroke: f32,
    minimap_w: f32,
) -> Vec<GroupRects> {
    compute_layout_geometry(split, area, stroke)
        .pane_rects
        .into_iter()
        .map(|editor| {
            let [x, y, w, h] = editor;
            if minimap_w <= 0.0 || w < 2.0 * minimap_w {
                return GroupRects {
                    editor,
                    text: editor,
                    minimap: None,
                };
            }
            GroupRects {
                editor,
                text: [x, y, w - minimap_w, h],
                minimap: Some([x + w - minimap_w, y, minimap_w, h]),
            }
        })
        .collect()
}

/// The groups of the current split, or one group for the single layout.
pub(crate) fn rects(app: &App) -> Vec<GroupRects> {
    layout(
        &app.ui.split,
        editor_area(app),
        app.scale_factor,
        app.minimap_width(),
    )
}

/// The group whose editor column holds `(x, y)`.
pub(crate) fn group_at(app: &App, x: f32, y: f32) -> Option<usize> {
    rects(app)
        .iter()
        .position(|group| contains(group.editor, x, y))
}

impl App {
    /// The focused group's gutter and text, where its clicks land.
    pub(crate) fn active_group_text_rect(&self) -> [f32; 4] {
        let groups = rects(self);
        groups
            .get(self.ui.split.active_pane)
            .or(groups.first())
            .map_or(editor_area(self), |group| group.text)
    }
}

pub(crate) fn contains([rx, ry, rw, rh]: [f32; 4], x: f32, y: f32) -> bool {
    x >= rx && x < rx + rw && y >= ry && y < ry + rh
}

#[cfg(test)]
#[path = "groups_tests.rs"]
mod tests;
