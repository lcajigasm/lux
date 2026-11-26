//! Where the editor, the find bar and the bottom dock sit in a window.
//!
//! One computation feeds the viewport, the painters and the hit tests, so a
//! panel that opens shrinks the editor in the same frame and a click lands
//! on what was drawn. The sidebar is a full-height column, as in Sublime
//! Text: the find bar and the dock span the editor column (minimap
//! included), not the sidebar.
//!
//! Top to bottom: tab bar, editor, find bar, dock, status bar.

use crate::app::App;

/// Heights and widths of the chrome around the editor, in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ChromeSizes {
    pub window_width: f32,
    pub window_height: f32,
    pub tab_bar: f32,
    pub status_bar: f32,
    pub search_bar: f32,
    pub dock: f32,
    pub sidebar: f32,
    pub minimap: f32,
}

/// Rectangles are `[x, y, width, height]` in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FrameLayout {
    /// The text area: right of the sidebar, left of the minimap.
    pub editor: [f32; 4],
    /// The editor column: the text area plus the minimap.
    pub column: [f32; 4],
    pub search_bar: Option<[f32; 4]>,
    pub dock: Option<[f32; 4]>,
}

pub(crate) fn compute(sizes: &ChromeSizes) -> FrameLayout {
    let left = sizes.sidebar.min(sizes.window_width);
    let column_width = (sizes.window_width - left).max(1.0);
    let editor_width = (column_width - sizes.minimap).max(1.0);
    let top = sizes.tab_bar;
    let below = sizes.search_bar + sizes.dock + sizes.status_bar;
    let editor_height = (sizes.window_height - top - below).max(1.0);
    let editor_bottom = top + editor_height;
    let search_bar =
        (sizes.search_bar > 0.0).then_some([left, editor_bottom, column_width, sizes.search_bar]);
    let dock = (sizes.dock > 0.0).then_some([
        left,
        editor_bottom + sizes.search_bar,
        column_width,
        sizes.dock,
    ]);
    FrameLayout {
        editor: [left, top, editor_width, editor_height],
        column: [left, top, column_width, editor_height],
        search_bar,
        dock,
    }
}

impl App {
    pub(crate) fn chrome_sizes(&self, window_width: f32, window_height: f32) -> ChromeSizes {
        ChromeSizes {
            window_width,
            window_height,
            tab_bar: self.tab_bar_height(),
            status_bar: self.status_bar_height(),
            search_bar: self.search_bar_height(),
            dock: self.term_dock_height(),
            sidebar: self.sidebar_width(),
            minimap: self.minimap_width(),
        }
    }

    /// The window's physical size. Without a window (tests, headless) it
    /// is the size the current viewport implies.
    pub(crate) fn window_extent(&self) -> (f32, f32) {
        if let Some(gpu) = &self.gpu_window {
            let size = gpu.window.inner_size();
            return (size.width as f32, size.height as f32);
        }
        let s = self.chrome_sizes(0.0, 0.0);
        (
            s.sidebar + self.viewport.width + s.minimap,
            s.tab_bar + self.viewport.height + s.search_bar + s.dock + s.status_bar,
        )
    }

    /// The layout of the live window, as the next frame paints it.
    pub(crate) fn frame_layout(&self) -> FrameLayout {
        let (width, height) = self.window_extent();
        compute(&self.chrome_sizes(width, height))
    }

    /// Sizes the editor viewport for a `width` × `height` frame.
    pub(crate) fn fit_viewport(&mut self, width: f32, height: f32) -> FrameLayout {
        let layout = compute(&self.chrome_sizes(width, height));
        self.viewport.width = layout.editor[2];
        self.viewport.height = layout.editor[3];
        layout
    }
}

#[cfg(test)]
#[path = "frame_layout_tests.rs"]
mod tests;
