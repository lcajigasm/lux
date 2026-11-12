//! Geometry for the settings page. Hit testing and painting both use this
//! so a row, its selection, and its control stay in one place.

use super::panel::{SettingKind, SettingsPanelState};

/// Logical sizes, multiplied by the display scale. The page fills the
/// editor well; these paddings are the same on every row.
const PAD: f32 = 28.0;
const HEADER: f32 = 56.0;
const SIDEBAR_W: f32 = 188.0;
const SEARCH_H: f32 = 32.0;
const CAT_H: f32 = 34.0;
const ROW: f32 = 72.0;
const GAP: f32 = 8.0;
const CONTROL_H: f32 = 30.0;
const TOGGLE_H: f32 = 24.0;
const SEL_INSET: f32 = 12.0;
const LABEL_GAP: f32 = 20.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub(crate) fn right(self) -> f32 {
        self.x + self.w
    }

    pub(crate) fn bottom(self) -> f32 {
        self.y + self.h
    }

    #[cfg(test)]
    pub(crate) fn center_y(self) -> f32 {
        self.y + self.h * 0.5
    }

    pub(crate) fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }

    #[cfg(test)]
    pub(crate) fn contains_rect(self, other: Rect) -> bool {
        other.w >= 0.0
            && other.h >= 0.0
            && other.x >= self.x - 0.05
            && other.y >= self.y - 0.05
            && other.right() <= self.right() + 0.05
            && other.bottom() <= self.bottom() + 0.05
    }

    pub(crate) fn intersection(self, other: Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right - x < 0.5 || bottom - y < 0.5 {
            None
        } else {
            Some(Rect {
                x,
                y,
                w: right - x,
                h: bottom - y,
            })
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SettingsBounds {
    pub window_width: f32,
    pub window_height: f32,
    pub tab_bar: f32,
    pub status_bar: f32,
    pub scale: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ControlGeom {
    Toggle {
        track: Rect,
        knob: Rect,
    },
    Stepper {
        minus: Rect,
        value: Rect,
        plus: Rect,
    },
    Menu {
        button: Rect,
    },
    None,
}

#[derive(Clone, Debug)]
pub(crate) struct RowGeom {
    pub index: usize,
    pub row: Rect,
    pub selection: Rect,
    pub label: Rect,
    pub description: Rect,
    /// Right-aligned control. Every row shares `control.right()`.
    pub control: Rect,
    pub kind: ControlGeom,
}

#[derive(Clone, Debug)]
pub(crate) struct MenuGeom {
    pub item: usize,
    pub panel: Rect,
    /// Index of `options[0]` in the enum. The rest of the list is scrolled.
    pub first: usize,
    pub options: Vec<Rect>,
}

/// Keeps `highlight` inside the visible window.
pub(crate) fn reveal_menu(highlight: usize, scroll: usize, visible: usize) -> usize {
    let visible = visible.max(1);
    if highlight < scroll {
        highlight
    } else if highlight >= scroll.saturating_add(visible) {
        highlight + 1 - visible
    } else {
        scroll
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SettingsLayout {
    pub page: Rect,
    pub body: Rect,
    pub sidebar: Rect,
    pub search: Rect,
    pub categories: Vec<Rect>,
    pub scrollbar: Option<Rect>,
    pub scroll: f32,
    pub scale: f32,
    pub title_x: f32,
    pub title_y: f32,
    pub hint_x: f32,
    pub hint_y: f32,
    pub rows: Vec<RowGeom>,
    pub menu: Option<MenuGeom>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SettingsHit {
    Outside,
    Page,
    Row(usize),
    Toggle(usize),
    StepMinus(usize),
    StepPlus(usize),
    MenuButton(usize),
    MenuOption { item: usize, option: usize },
    Category(usize),
    Search,
}

pub(crate) fn layout_settings(
    panel: &SettingsPanelState,
    bounds: SettingsBounds,
) -> SettingsLayout {
    let scale = if bounds.scale.is_finite() && bounds.scale > 0.0 {
        bounds.scale
    } else {
        1.0
    };
    let s = |logical: f32| logical * scale;
    let width = bounds.window_width.max(1.0);
    let height = bounds.window_height.max(1.0);
    let page = Rect {
        x: 0.0,
        y: bounds.tab_bar,
        w: width,
        h: (height - bounds.tab_bar - bounds.status_bar).max(1.0),
    };
    let header = s(HEADER);
    let sidebar = Rect {
        x: page.x,
        y: page.y + header,
        w: s(SIDEBAR_W),
        h: (page.h - header).max(1.0),
    };
    let search = Rect {
        x: sidebar.right() + s(16.0),
        y: page.y + header,
        w: (page.right() - sidebar.right() - s(16.0) - s(PAD)).max(1.0),
        h: s(SEARCH_H),
    };
    let body = Rect {
        x: search.x,
        y: search.bottom() + s(12.0),
        w: (search.w - s(12.0)).max(1.0),
        h: (page.bottom() - search.bottom() - s(24.0)).max(1.0),
    };
    let categories = panel
        .categories()
        .into_iter()
        .enumerate()
        .map(|(index, _)| Rect {
            x: sidebar.x + s(12.0),
            y: sidebar.y + s(12.0) + index as f32 * s(CAT_H),
            w: (sidebar.w - s(24.0)).max(1.0),
            h: s(CAT_H - 4.0),
        })
        .collect::<Vec<_>>();
    let row_h = s(ROW);
    let gap = s(GAP);
    let visible = panel.visible_indices();
    let count = visible.len();
    let content_h = if count == 0 {
        0.0
    } else {
        count as f32 * row_h + (count - 1) as f32 * gap
    };
    let max_scroll = (content_h - body.h).max(0.0);
    let scroll = panel.scroll.clamp(0.0, max_scroll);
    let scrollbar = if max_scroll > 0.5 {
        let track_h = body.h;
        let thumb_h = (track_h * (body.h / content_h.max(1.0))).clamp(s(24.0), track_h);
        let travel = (track_h - thumb_h).max(0.0);
        let thumb_y = body.y
            + if max_scroll <= 0.0 {
                0.0
            } else {
                travel * (scroll / max_scroll)
            };
        Some(Rect {
            x: body.right() + s(4.0),
            y: thumb_y,
            w: s(4.0),
            h: thumb_h,
        })
    } else {
        None
    };
    let mut rows = Vec::with_capacity(count);
    let mut y = body.y - scroll;
    for index in visible {
        let Some(item) = panel.items.get(index) else {
            continue;
        };
        let row = Rect {
            x: body.x,
            y,
            w: body.w,
            h: row_h,
        };
        let (control, kind) = control_geom(&item.kind, row, body, scale);
        let label_w = (control.x - s(LABEL_GAP) - row.x).max(s(40.0));
        let label = Rect {
            x: row.x,
            y: row.y + s(12.0),
            w: label_w,
            h: s(18.0),
        };
        let description = Rect {
            x: label.x,
            y: label.bottom() + s(4.0),
            w: label.w,
            h: s(18.0),
        };
        let selection = Rect {
            x: page.x + s(SEL_INSET),
            y: row.y + s(4.0),
            w: (page.w - s(SEL_INSET) * 2.0).max(1.0),
            h: (row.h - s(8.0)).max(1.0),
        };
        rows.push(RowGeom {
            index,
            row,
            selection,
            label,
            description,
            control,
            kind,
        });
        y += row_h + gap;
    }
    let menu = panel.menu.as_ref().and_then(|menu| {
        let row = rows.iter().find(|row| row.index == menu.item)?;
        let SettingKind::Enum { options, .. } = &panel.items.get(menu.item)?.kind else {
            return None;
        };
        Some(menu_geom(row, options.len(), menu.scroll, page, scale))
    });
    SettingsLayout {
        page,
        body,
        sidebar,
        search,
        categories,
        scrollbar,
        scroll,
        scale,
        title_x: page.x + s(PAD),
        title_y: page.y + s(18.0),
        hint_x: search.x,
        hint_y: page.y + s(18.0),
        rows,
        menu,
    }
}

fn control_geom(kind: &SettingKind, row: Rect, body: Rect, scale: f32) -> (Rect, ControlGeom) {
    let s = |logical: f32| logical * scale;
    if matches!(kind, SettingKind::Info) {
        let height = s(CONTROL_H);
        return (
            Rect {
                x: body.right(),
                y: row.y + (row.h - height) * 0.5,
                w: 0.0,
                h: height,
            },
            ControlGeom::None,
        );
    }
    let height = match kind {
        SettingKind::Boolean { .. } => s(TOGGLE_H),
        _ => s(CONTROL_H),
    };
    let width = match kind {
        SettingKind::Boolean { .. } => s(46.0),
        SettingKind::Number { .. } => s(156.0),
        SettingKind::Enum { .. } => s(196.0),
        SettingKind::Info => 0.0,
    };
    let control = Rect {
        x: body.right() - width,
        y: row.y + (row.h - height) * 0.5,
        w: width,
        h: height,
    };
    let geom = match kind {
        SettingKind::Boolean { value } => {
            let knob_d = s(18.0);
            let pad = s(3.0);
            let knob_x = if *value {
                control.right() - pad - knob_d
            } else {
                control.x + pad
            };
            ControlGeom::Toggle {
                track: control,
                knob: Rect {
                    x: knob_x,
                    y: control.y + (control.h - knob_d) * 0.5,
                    w: knob_d,
                    h: knob_d,
                },
            }
        }
        SettingKind::Number { .. } => {
            let button = control.h;
            let minus = Rect {
                x: control.x,
                y: control.y,
                w: button,
                h: control.h,
            };
            let plus = Rect {
                x: control.right() - button,
                y: control.y,
                w: button,
                h: control.h,
            };
            ControlGeom::Stepper {
                minus,
                value: Rect {
                    x: minus.right(),
                    y: control.y,
                    w: (plus.x - minus.right()).max(1.0),
                    h: control.h,
                },
                plus,
            }
        }
        SettingKind::Enum { .. } => ControlGeom::Menu { button: control },
        SettingKind::Info => ControlGeom::None,
    };
    (control, geom)
}

fn menu_geom(
    row: &RowGeom,
    option_count: usize,
    scroll: usize,
    page: Rect,
    scale: f32,
) -> MenuGeom {
    let s = |logical: f32| logical * scale;
    let option_h = s(30.0);
    let pad = s(4.0);
    let limit = (page.h - s(16.0)).max(option_h);
    let capacity = ((limit - pad * 2.0) / option_h).floor().max(1.0) as usize;
    let visible = capacity.min(option_count.max(1));
    let first = scroll.min(option_count.saturating_sub(visible));
    let panel_h = visible as f32 * option_h + pad * 2.0;
    let button = row.control;
    let mut menu_y = button.bottom() + s(4.0);
    if menu_y + panel_h > page.bottom() - s(8.0) {
        let above = button.y - s(4.0) - panel_h;
        menu_y = if above >= page.y + s(8.0) {
            above
        } else {
            page.y + s(8.0)
        };
    }
    if menu_y + panel_h > page.bottom() - s(8.0) {
        menu_y = (page.bottom() - s(8.0) - panel_h).max(page.y + s(8.0));
    }
    let panel = Rect {
        x: button.x,
        y: menu_y,
        w: button.w,
        h: panel_h,
    };
    let options = (0..visible)
        .map(|offset| Rect {
            x: panel.x + pad,
            y: panel.y + pad + offset as f32 * option_h,
            w: (panel.w - pad * 2.0).max(1.0),
            h: option_h,
        })
        .collect();
    MenuGeom {
        item: row.index,
        panel,
        first,
        options,
    }
}

impl SettingsLayout {
    /// Selection actually painted. A row that has scrolled out of the body
    /// contributes nothing, so the highlight cannot cross the page edge.
    pub(crate) fn painted_selection(&self, row: &RowGeom) -> Option<Rect> {
        row.selection
            .intersection(self.body)
            .and_then(|rect| rect.intersection(self.page))
    }

    pub(crate) fn row_is_visible(&self, row: &RowGeom) -> bool {
        row.label.y >= self.body.y - 0.5 && row.description.bottom() <= self.body.bottom() + 0.5
    }

    pub(crate) fn hit(&self, x: f32, y: f32) -> SettingsHit {
        if let Some(menu) = &self.menu {
            for (offset, rect) in menu.options.iter().enumerate() {
                if rect.contains(x, y) {
                    return SettingsHit::MenuOption {
                        item: menu.item,
                        option: menu.first + offset,
                    };
                }
            }
            if menu.panel.contains(x, y) {
                return SettingsHit::Page;
            }
        }
        if self.search.contains(x, y) {
            return SettingsHit::Search;
        }
        for (index, rect) in self.categories.iter().enumerate() {
            if rect.contains(x, y) {
                return SettingsHit::Category(index);
            }
        }
        for row in &self.rows {
            if row.row.intersection(self.body).is_none() {
                continue;
            }
            match row.kind {
                ControlGeom::Toggle { track, .. } if track.contains(x, y) => {
                    return SettingsHit::Toggle(row.index);
                }
                ControlGeom::Stepper { minus, plus, .. } => {
                    if minus.contains(x, y) {
                        return SettingsHit::StepMinus(row.index);
                    }
                    if plus.contains(x, y) {
                        return SettingsHit::StepPlus(row.index);
                    }
                }
                ControlGeom::Menu { button } if button.contains(x, y) => {
                    return SettingsHit::MenuButton(row.index);
                }
                _ => {}
            }
            if row.row.contains(x, y) && self.body.contains(x, y) {
                return SettingsHit::Row(row.index);
            }
        }
        if self.page.contains(x, y) {
            SettingsHit::Page
        } else {
            SettingsHit::Outside
        }
    }
}

/// A setting value as the stepper shows it. Whole numbers drop the fraction.
pub(crate) fn format_number(value: f64) -> String {
    if (value - value.round()).abs() < 0.001 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}")
    }
}
