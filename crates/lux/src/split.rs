//! Split editor layout: multi-column, multi-row, and 2x2 grid layouts (T04, ST12).

use crate::app::App;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum LayoutPreset {
    #[default]
    Single,
    Columns2,
    Columns3,
    Columns4,
    Rows2,
    Rows3,
    Grid2x2,
}

impl LayoutPreset {
    pub(crate) fn num_panes(&self) -> usize {
        match self {
            Self::Single => 1,
            Self::Columns2 | Self::Rows2 => 2,
            Self::Columns3 | Self::Rows3 => 3,
            Self::Columns4 | Self::Grid2x2 => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SplitDirection {
    Horizontal,
    Vertical,
}

pub(crate) mod clone_view;
pub(crate) mod groups;
pub(crate) mod tabs;
pub(crate) use crate::pane_view::PaneView;
#[allow(unused_imports)]
pub(crate) use crate::pane_view::PaneViewport;

#[allow(dead_code)]
pub(crate) type SplitPane = PaneView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DraggingDivider {
    Vertical(usize),
    Horizontal(usize),
}

#[derive(Debug, Clone)]
pub(crate) struct SplitLayout {
    pub(crate) preset: LayoutPreset,
    pub(crate) direction: SplitDirection,
    pub(crate) ratio: f32,
    pub(crate) ratio_y: f32,
    pub(crate) secondary: Option<PaneView>,
    pub(crate) active_pane: usize,
    pub(crate) panes: Vec<PaneView>,
    pub(crate) dragging_divider: Option<DraggingDivider>,
    /// What `split::tabs::reconcile` last brought the groups in line with.
    pub(crate) reconciled: Option<u64>,
}

impl Default for SplitLayout {
    fn default() -> Self {
        Self {
            preset: LayoutPreset::Single,
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            ratio_y: 0.5,
            secondary: None,
            active_pane: 0,
            panes: Vec::new(),
            dragging_divider: None,
            reconciled: None,
        }
    }
}

impl SplitLayout {
    pub(crate) fn is_active(&self) -> bool {
        self.preset != LayoutPreset::Single || self.secondary.is_some()
    }

    pub(crate) fn num_panes(&self) -> usize {
        if self.preset == LayoutPreset::Single && self.secondary.is_some() {
            2
        } else {
            self.preset.num_panes()
        }
    }
}

pub(crate) struct LayoutGeometry {
    pub(crate) pane_rects: Vec<[f32; 4]>,
    pub(crate) dividers: Vec<([f32; 4], DraggingDivider)>,
}

pub(crate) fn compute_layout_geometry(
    split: &SplitLayout,
    primary_rect: [f32; 4],
    stroke: f32,
) -> LayoutGeometry {
    let [px, py, pw, ph] = primary_rect;
    let thickness = 2.0 * stroke;
    let mut pane_rects = Vec::new();
    let mut dividers = Vec::new();

    let preset = if split.preset == LayoutPreset::Single && split.secondary.is_some() {
        match split.direction {
            SplitDirection::Horizontal => LayoutPreset::Columns2,
            SplitDirection::Vertical => LayoutPreset::Rows2,
        }
    } else {
        split.preset
    };

    match preset {
        LayoutPreset::Single => {
            pane_rects.push([px, py, pw, ph]);
        }
        LayoutPreset::Columns2 => {
            let rx = split.ratio.clamp(0.1, 0.9);
            let div_x = px + pw * rx - thickness * 0.5;
            let w0 = (pw * rx - thickness * 0.5).max(1.0);
            let w1 = (pw - pw * rx - thickness * 0.5).max(1.0);
            pane_rects.push([px, py, w0, ph]);
            pane_rects.push([div_x + thickness, py, w1, ph]);
            dividers.push(([div_x, py, thickness, ph], DraggingDivider::Vertical(0)));
        }
        LayoutPreset::Columns3 => {
            let total_gap = 2.0 * thickness;
            let col_w = (pw - total_gap).max(1.0) / 3.0;
            pane_rects.push([px, py, col_w, ph]);
            dividers.push((
                [px + col_w, py, thickness, ph],
                DraggingDivider::Vertical(0),
            ));
            pane_rects.push([px + col_w + thickness, py, col_w, ph]);
            dividers.push((
                [px + 2.0 * col_w + thickness, py, thickness, ph],
                DraggingDivider::Vertical(1),
            ));
            pane_rects.push([
                px + 2.0 * col_w + 2.0 * thickness,
                py,
                pw - (2.0 * col_w + 2.0 * thickness),
                ph,
            ]);
        }
        LayoutPreset::Columns4 => {
            let total_gap = 3.0 * thickness;
            let col_w = (pw - total_gap).max(1.0) / 4.0;
            for i in 0..4 {
                let x = px + i as f32 * (col_w + thickness);
                let w = if i == 3 {
                    pw - (3.0 * (col_w + thickness))
                } else {
                    col_w
                };
                pane_rects.push([x, py, w.max(1.0), ph]);
                if i < 3 {
                    dividers.push(([x + col_w, py, thickness, ph], DraggingDivider::Vertical(i)));
                }
            }
        }
        LayoutPreset::Rows2 => {
            let ry = split.ratio_y.clamp(0.1, 0.9);
            let div_y = py + ph * ry - thickness * 0.5;
            let h0 = (ph * ry - thickness * 0.5).max(1.0);
            let h1 = (ph - ph * ry - thickness * 0.5).max(1.0);
            pane_rects.push([px, py, pw, h0]);
            pane_rects.push([px, div_y + thickness, pw, h1]);
            dividers.push(([px, div_y, pw, thickness], DraggingDivider::Horizontal(0)));
        }
        LayoutPreset::Rows3 => {
            let total_gap = 2.0 * thickness;
            let row_h = (ph - total_gap).max(1.0) / 3.0;
            pane_rects.push([px, py, pw, row_h]);
            dividers.push((
                [px, py + row_h, pw, thickness],
                DraggingDivider::Horizontal(0),
            ));
            pane_rects.push([px, py + row_h + thickness, pw, row_h]);
            dividers.push((
                [px, py + 2.0 * row_h + thickness, pw, thickness],
                DraggingDivider::Horizontal(1),
            ));
            pane_rects.push([
                px,
                py + 2.0 * row_h + 2.0 * thickness,
                pw,
                ph - (2.0 * row_h + 2.0 * thickness),
            ]);
        }
        LayoutPreset::Grid2x2 => {
            let rx = split.ratio.clamp(0.1, 0.9);
            let ry = split.ratio_y.clamp(0.1, 0.9);
            let div_x = px + pw * rx - thickness * 0.5;
            let div_y = py + ph * ry - thickness * 0.5;
            let w_left = (pw * rx - thickness * 0.5).max(1.0);
            let w_right = (pw - pw * rx - thickness * 0.5).max(1.0);
            let h_top = (ph * ry - thickness * 0.5).max(1.0);
            let h_bottom = (ph - ph * ry - thickness * 0.5).max(1.0);

            pane_rects.push([px, py, w_left, h_top]);
            pane_rects.push([div_x + thickness, py, w_right, h_top]);
            pane_rects.push([px, div_y + thickness, w_left, h_bottom]);
            pane_rects.push([div_x + thickness, div_y + thickness, w_right, h_bottom]);

            dividers.push(([div_x, py, thickness, ph], DraggingDivider::Vertical(0)));
            dividers.push(([px, div_y, pw, thickness], DraggingDivider::Horizontal(0)));
        }
    }

    LayoutGeometry {
        pane_rects,
        dividers,
    }
}

pub(crate) fn hit_test_divider(
    split: &SplitLayout,
    primary_rect: [f32; 4],
    stroke: f32,
    mouse_x: f32,
    mouse_y: f32,
) -> Option<DraggingDivider> {
    let geom = compute_layout_geometry(split, primary_rect, stroke);
    for (rect, div_type) in geom.dividers {
        let [x, y, w, h] = rect;
        let hit_rect = [x - 4.0, y - 4.0, w + 8.0, h + 8.0];
        if mouse_x >= hit_rect[0]
            && mouse_x <= hit_rect[0] + hit_rect[2]
            && mouse_y >= hit_rect[1]
            && mouse_y <= hit_rect[1] + hit_rect[3]
        {
            return Some(div_type);
        }
    }
    None
}

#[cfg(test)]
pub(crate) fn hit_test_pane(
    split: &SplitLayout,
    primary_rect: [f32; 4],
    stroke: f32,
    mouse_x: f32,
    mouse_y: f32,
) -> Option<usize> {
    let geom = compute_layout_geometry(split, primary_rect, stroke);
    for (i, &[x, y, w, h]) in geom.pane_rects.iter().enumerate() {
        if mouse_x >= x && mouse_x <= x + w && mouse_y >= y && mouse_y <= y + h {
            return Some(i);
        }
    }
    None
}

impl App {
    pub(crate) fn split_editor_right(&mut self) {
        self.split_editor(SplitDirection::Horizontal);
    }

    pub(crate) fn split_editor_down(&mut self) {
        self.split_editor(SplitDirection::Vertical);
    }

    pub(crate) fn split_editor(&mut self, direction: SplitDirection) {
        self.ui.split.direction = direction;
        match direction {
            SplitDirection::Horizontal => self.set_layout(LayoutPreset::Columns2),
            SplitDirection::Vertical => self.set_layout(LayoutPreset::Rows2),
        }
    }

    pub(crate) fn close_active_split(&mut self) -> bool {
        if !self.ui.split.is_active() {
            return false;
        }
        let next_preset = match self.ui.split.preset {
            LayoutPreset::Columns4 => LayoutPreset::Columns3,
            LayoutPreset::Columns3 => LayoutPreset::Columns2,
            LayoutPreset::Columns2 => LayoutPreset::Single,
            LayoutPreset::Rows3 => LayoutPreset::Rows2,
            LayoutPreset::Rows2 => LayoutPreset::Single,
            LayoutPreset::Grid2x2 => LayoutPreset::Columns2,
            LayoutPreset::Single => LayoutPreset::Single,
        };
        self.set_layout(next_preset);
        true
    }

    pub(crate) fn focus_next_split(&mut self) -> bool {
        let total = self.ui.split.num_panes();
        if total <= 1 {
            return false;
        }
        let next = (self.ui.split.active_pane + 1) % total;
        self.focus_group(next)
    }

    pub(crate) fn focus_group(&mut self, group_idx: usize) -> bool {
        let num_panes = self.ui.split.num_panes();
        if group_idx >= num_panes {
            return false;
        }
        tabs::store_active_pane(self);
        tabs::enter_pane(self, group_idx);
        self.ui.split.secondary = self.ui.split.panes.get(1).cloned();
        self.scheduler.invalidate_all();
        true
    }

    pub(crate) fn handle_divider_drag(&mut self, mouse_x: f32, mouse_y: f32) {
        if let Some(div) = self.ui.split.dragging_divider {
            let [left, top, width, height] = groups::editor_area(self);
            match div {
                DraggingDivider::Vertical(_) => {
                    self.ui.split.ratio = ((mouse_x - left) / width.max(1.0)).clamp(0.1, 0.9);
                }
                DraggingDivider::Horizontal(_) => {
                    self.ui.split.ratio_y = ((mouse_y - top) / height.max(1.0)).clamp(0.1, 0.9);
                }
            }
            self.scheduler.invalidate_all();
        }
    }

    pub(crate) fn end_divider_drag(&mut self) {
        self.ui.split.dragging_divider = None;
    }
}

#[cfg(test)]
mod clone_view_tests;
#[cfg(test)]
mod tests;
