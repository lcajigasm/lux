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
        let mut primary_pane = PaneView::new(active_tab);
        primary_pane.cursors = self.cursors.clone();
        primary_pane.fold_state = self.fold_state.clone();
        primary_pane.bookmarks = self.bookmarks.clone();
        primary_pane.viewport.scroll_x = self.viewport.scroll_x;
        primary_pane.viewport.scroll_y = self.viewport.scroll_y;
        if let Some(syntax) = &self.syntax {
            primary_pane.tokens = syntax.tokens.clone();
        }
        new_panes.push(primary_pane);

        let mut tab_iter = tabs.iter().filter(|t| t.id != active_tab);
        for _ in 1..needed {
            let tab_id = tab_iter.next().map(|t| t.id).unwrap_or(active_tab);
            let mut pane = PaneView::new(tab_id);
            if tab_id == active_tab {
                pane.cursors = self.cursors.clone();
                pane.fold_state = self.fold_state.clone();
                pane.bookmarks = self.bookmarks.clone();
                pane.viewport.scroll_x = self.viewport.scroll_x;
                pane.viewport.scroll_y = self.viewport.scroll_y;
                if let Some(syntax) = &self.syntax {
                    pane.tokens = syntax.tokens.clone();
                }
            } else if let Some(entry) = self.documents.entries.iter().find(|e| e.tab_id == tab_id) {
                pane.cursors = entry.cursors.clone();
                pane.fold_state = entry.fold_state.clone();
                pane.bookmarks = entry.bookmarks.clone();
                pane.viewport.scroll_x = entry.scroll_x;
                pane.viewport.scroll_y = entry.scroll_y;
                let lang = entry.language.or_else(|| {
                    crate::document::detect_language(entry.path.as_deref(), &entry.buffer)
                });
                if let Some(lang) = lang {
                    let mut doc = lux_syntax::Document::new(lang);
                    doc.set_rope(entry.buffer.rope());
                    pane.tokens = doc.highlight_rope(entry.buffer.rope());
                }
            }
            new_panes.push(pane);
        }

        let sb_w = self.sidebar_width();
        let top = self.tab_bar_height();
        let geom = compute_layout_geometry(
            &self.ui.split,
            [sb_w, top, self.viewport.width, self.viewport.height],
            self.scale_factor,
        );
        for (pane, &rect) in new_panes.iter_mut().zip(geom.pane_rects.iter()) {
            pane.viewport.x = rect[0];
            pane.viewport.y = rect[1];
            pane.viewport.width = rect[2];
            pane.viewport.height = rect[3];
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
        let cur_active = self.ui.split.active_pane;
        if cur_active < self.ui.split.panes.len() {
            self.ui.split.panes[cur_active].cursors = self.cursors.clone();
            self.ui.split.panes[cur_active].fold_state = self.fold_state.clone();
            self.ui.split.panes[cur_active].bookmarks = self.bookmarks.clone();
            self.ui.split.panes[cur_active].viewport.scroll_x = self.viewport.scroll_x;
            self.ui.split.panes[cur_active].viewport.scroll_y = self.viewport.scroll_y;
            if let Some(syntax) = &self.syntax {
                self.ui.split.panes[cur_active].tokens = syntax.tokens.clone();
            }
        }
        self.ui.split.active_pane = group_idx;
        if let Some(pane) = self.ui.split.panes.get(group_idx) {
            let target = pane.tab_id;
            let pane_cursors = pane.cursors.clone();
            let scroll_x = pane.viewport.scroll_x;
            let scroll_y = pane.viewport.scroll_y;
            let fold_state = pane.fold_state.clone();
            let bookmarks = pane.bookmarks.clone();
            self.switch_to_tab(target);
            let doc_end = lux_core::position::Position::document_end(&self.buffer);
            let clamped: Vec<_> = pane_cursors
                .cursors()
                .iter()
                .map(|s| {
                    let a = s.anchor.char.min(doc_end.char);
                    let h = s.head.char.min(doc_end.char);
                    lux_core::selection::Selection::new(
                        lux_core::position::Position::from_char(&self.buffer, a),
                        lux_core::position::Position::from_char(&self.buffer, h),
                    )
                })
                .collect();
            self.cursors = lux_core::multicursor::MultiCursor::from_cursors(clamped);
            self.viewport.scroll_x = scroll_x;
            self.viewport.scroll_y = scroll_y;
            self.fold_state = fold_state;
            self.bookmarks = bookmarks;
            self.reset_goal_columns();
        }
        self.ui.split.secondary = self.ui.split.panes.get(1).cloned();
        self.scheduler.invalidate_all();
        true
    }

    pub(crate) fn move_to_group(&mut self, group_idx: usize) -> bool {
        let num_panes = self.ui.split.num_panes();
        if group_idx >= num_panes {
            return false;
        }
        let cur_pane = self.ui.split.active_pane;
        if cur_pane == group_idx {
            return true;
        }
        if let Some(tab_id) = self.documents.active_tab {
            if cur_pane < self.ui.split.panes.len() {
                self.ui.split.panes[cur_pane].cursors = self.cursors.clone();
                self.ui.split.panes[cur_pane].viewport.scroll_x = self.viewport.scroll_x;
                self.ui.split.panes[cur_pane].viewport.scroll_y = self.viewport.scroll_y;
            }
            if group_idx < self.ui.split.panes.len() {
                self.ui.split.panes[group_idx].tab_id = tab_id;
                self.ui.split.panes[group_idx].active_tab = tab_id;
                self.ui.split.panes[group_idx].cursors = self.cursors.clone();
                self.ui.split.panes[group_idx].viewport.scroll_x = self.viewport.scroll_x;
                self.ui.split.panes[group_idx].viewport.scroll_y = self.viewport.scroll_y;

                let other_tab = self
                    .documents
                    .tabs
                    .tabs()
                    .iter()
                    .map(|t| t.id)
                    .find(|&id| {
                        id != tab_id
                            && !self
                                .ui
                                .split
                                .panes
                                .iter()
                                .enumerate()
                                .any(|(i, p)| i != cur_pane && p.tab_id == id)
                    })
                    .or_else(|| {
                        self.documents
                            .tabs
                            .tabs()
                            .iter()
                            .map(|t| t.id)
                            .find(|&id| id != tab_id)
                    });

                if let Some(other) = other_tab {
                    self.ui.split.panes[cur_pane].tab_id = other;
                    self.ui.split.panes[cur_pane].active_tab = other;
                    if let Some(entry) = self.documents.entries.iter().find(|e| e.tab_id == other) {
                        self.ui.split.panes[cur_pane].cursors = entry.cursors.clone();
                        self.ui.split.panes[cur_pane].viewport.scroll_x = entry.scroll_x;
                        self.ui.split.panes[cur_pane].viewport.scroll_y = entry.scroll_y;
                    }
                }
            }
            if group_idx == 1 || cur_pane == 1 {
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
mod clone_view_tests;
#[cfg(test)]
mod tests;
