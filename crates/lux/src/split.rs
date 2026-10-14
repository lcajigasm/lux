//! Split editor layout: multi-column, multi-row, and 2x2 grid layouts (T04, ST12).

use lux_ui::tabs::TabId;

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

#[derive(Debug, Clone)]
pub(crate) struct SplitPane {
    pub(crate) tab_id: TabId,
    #[allow(dead_code)]
    pub(crate) scroll_x: f32,
    #[allow(dead_code)]
    pub(crate) scroll_y: f32,
}

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
    pub(crate) secondary: Option<SplitPane>,
    pub(crate) active_pane: usize,
    pub(crate) panes: Vec<SplitPane>,
    pub(crate) dragging_divider: Option<DraggingDivider>,
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

    pub(crate) fn set_layout(&mut self, preset: LayoutPreset) {
        let active_tab = self
            .documents
            .active_tab
            .or_else(|| self.documents.tabs.tabs().first().map(|t| t.id));
        let Some(active_tab) = active_tab else {
            return;
        };

        self.ui.split.preset = preset;
        if preset == LayoutPreset::Single {
            self.ui.split.secondary = None;
            self.ui.split.panes.clear();
            self.ui.split.active_pane = 0;
            self.update_viewport_dimensions();
            self.scheduler.invalidate_all();
            return;
        }

        let needed = preset.num_panes();
        let tabs = self.documents.tabs.tabs().to_vec();

        let mut new_panes = Vec::with_capacity(needed);
        new_panes.push(SplitPane {
            tab_id: active_tab,
            scroll_x: 0.0,
            scroll_y: 0.0,
        });

        let mut tab_iter = tabs.iter().filter(|t| t.id != active_tab);
        for _ in 1..needed {
            let tab_id = tab_iter.next().map(|t| t.id).unwrap_or(active_tab);
            new_panes.push(SplitPane {
                tab_id,
                scroll_x: 0.0,
                scroll_y: 0.0,
            });
        }

        self.ui.split.panes = new_panes;
        self.ui.split.secondary = self.ui.split.panes.get(1).cloned();
        self.ui.split.direction = match preset {
            LayoutPreset::Rows2 | LayoutPreset::Rows3 => SplitDirection::Vertical,
            _ => SplitDirection::Horizontal,
        };
        if self.ui.split.active_pane >= needed {
            self.ui.split.active_pane = 0;
        }
        self.update_viewport_dimensions();
        self.scheduler.invalidate_all();
    }

    pub(crate) fn close_active_split(&mut self) -> bool {
        if !self.ui.split.is_active() {
            return false;
        }
        self.set_layout(LayoutPreset::Single);
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
        self.ui.split.active_pane = group_idx;
        if let Some(pane) = self.ui.split.panes.get(group_idx) {
            let target = pane.tab_id;
            self.switch_to_tab(target);
        }
        self.scheduler.invalidate_all();
        true
    }

    pub(crate) fn move_to_group(&mut self, group_idx: usize) -> bool {
        let num_panes = self.ui.split.num_panes();
        if group_idx >= num_panes {
            return false;
        }
        if let Some(tab_id) = self.documents.active_tab {
            if group_idx < self.ui.split.panes.len() {
                self.ui.split.panes[group_idx].tab_id = tab_id;
            }
            if group_idx == 1 {
                self.ui.split.secondary = self.ui.split.panes.get(1).cloned();
            }
        }
        self.focus_group(group_idx)
    }

    pub(crate) fn handle_divider_drag(&mut self, mouse_x: f32, mouse_y: f32) {
        if let Some(div) = self.ui.split.dragging_divider {
            let sb_w = self.sidebar_width();
            let top = self.tab_bar_height();
            match div {
                DraggingDivider::Vertical(_) => {
                    self.ui.split.ratio = ((mouse_x - sb_w) / self.viewport.width).clamp(0.1, 0.9);
                }
                DraggingDivider::Horizontal(_) => {
                    self.ui.split.ratio_y =
                        ((mouse_y - top) / self.viewport.height).clamp(0.1, 0.9);
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
mod tests {
    use super::*;

    #[test]
    fn split_editor_creates_secondary_pane_and_switches_focus() {
        let mut app = App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());

        assert!(app.ui.split.secondary.is_none());
        assert_eq!(app.ui.split.active_pane, 0);

        // Split editor right
        app.split_editor_right();
        assert!(app.ui.split.secondary.is_some());
        assert_eq!(app.ui.split.direction, SplitDirection::Horizontal);
        assert_eq!(app.ui.split.active_pane, 0);

        // Focus next split cycles to pane 1
        assert!(app.focus_next_split());
        assert_eq!(app.ui.split.active_pane, 1);

        // Focus next split cycles back to pane 0
        assert!(app.focus_next_split());
        assert_eq!(app.ui.split.active_pane, 0);

        // Split editor down
        app.split_editor_down();
        assert_eq!(app.ui.split.direction, SplitDirection::Vertical);

        // Close split
        assert!(app.close_active_split());
        assert!(app.ui.split.secondary.is_none());
        assert_eq!(app.ui.split.active_pane, 0);
    }

    #[test]
    fn split_editor_syncs_same_buffer_edits() {
        let mut app = App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        app.insert_at_cursor("initial text");

        app.split_editor_right();
        let secondary = app.ui.split.secondary.as_ref().unwrap();
        assert_eq!(secondary.tab_id, app.documents.active_tab.unwrap());

        // Edit in current pane
        app.insert_at_cursor(" + edited");
        assert_eq!(app.buffer.text(), "initial text + edited");
    }

    #[test]
    fn layouts_and_groups_lifecycle() {
        let mut app = App::new(include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec());

        // Single layout initially
        assert_eq!(app.ui.split.preset, LayoutPreset::Single);
        assert_eq!(app.ui.split.num_panes(), 1);

        // 2 Columns
        app.set_layout(LayoutPreset::Columns2);
        assert_eq!(app.ui.split.num_panes(), 2);
        assert_eq!(app.ui.split.panes.len(), 2);

        // 3 Columns
        app.set_layout(LayoutPreset::Columns3);
        assert_eq!(app.ui.split.num_panes(), 3);
        assert_eq!(app.ui.split.panes.len(), 3);

        // 4 Columns
        app.set_layout(LayoutPreset::Columns4);
        assert_eq!(app.ui.split.num_panes(), 4);
        assert_eq!(app.ui.split.panes.len(), 4);

        // 2 Rows
        app.set_layout(LayoutPreset::Rows2);
        assert_eq!(app.ui.split.num_panes(), 2);

        // Grid 2x2
        app.set_layout(LayoutPreset::Grid2x2);
        assert_eq!(app.ui.split.num_panes(), 4);

        // Focus group 1..4
        assert!(app.focus_group(1));
        assert_eq!(app.ui.split.active_pane, 1);
        assert!(app.focus_group(3));
        assert_eq!(app.ui.split.active_pane, 3);
        assert!(!app.focus_group(4)); // out of bounds

        // Move to group
        assert!(app.move_to_group(2));
        assert_eq!(app.ui.split.active_pane, 2);

        // Back to Single
        app.set_layout(LayoutPreset::Single);
        assert_eq!(app.ui.split.num_panes(), 1);
        assert!(app.ui.split.panes.is_empty());
    }

    #[test]
    fn geometry_computation_and_divider_hit_testing() {
        let split = SplitLayout {
            preset: LayoutPreset::Grid2x2,
            ..Default::default()
        };
        let rect = [0.0, 0.0, 800.0, 600.0];
        let geom = compute_layout_geometry(&split, rect, 1.0);
        assert_eq!(geom.pane_rects.len(), 4);
        assert_eq!(geom.dividers.len(), 2);

        // Vertical divider is at x = ~400
        let hit_v = hit_test_divider(&split, rect, 1.0, 400.0, 300.0);
        assert_eq!(hit_v, Some(DraggingDivider::Vertical(0)));

        // Horizontal divider is at y = ~300
        let hit_h = hit_test_divider(&split, rect, 1.0, 200.0, 300.0);
        assert_eq!(hit_h, Some(DraggingDivider::Horizontal(0)));

        // Pane 0 is top-left
        let pane_0 = hit_test_pane(&split, rect, 1.0, 100.0, 100.0);
        assert_eq!(pane_0, Some(0));

        // Pane 3 is bottom-right
        let pane_3 = hit_test_pane(&split, rect, 1.0, 600.0, 500.0);
        assert_eq!(pane_3, Some(3));
    }
}
