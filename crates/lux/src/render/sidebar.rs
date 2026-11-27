//! Rendering logic for the file tree sidebar (G12).

use swash::FontRef;
use swash::scale::ScaleContext;

use super::panel::chrome_text::{self, ChromePen};
use crate::render::paint::color;
use crate::sidebar::{SIDEBAR_HEADER_HEIGHT, SIDEBAR_ROW_HEIGHT, SidebarState};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;
use lux_ui::file_tree::NodeKind;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_sidebar(
    sidebar: &mut SidebarState,
    sidebar_width: f32,
    tab_bar_height: f32,
    content_height: f32,
    scale_factor: f32,
    stroke: f32,
    font: FontRef<'static>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    theme: &lux_ui::Theme,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    if !sidebar.visible || sidebar_width <= 0.0 {
        return;
    }

    let root_title = sidebar.root_name().to_uppercase();

    let drop_target = sidebar
        .drag
        .as_ref()
        .filter(|drag| drag.active)
        .and_then(|drag| drag.drop_target);
    let Some(root_id) = sidebar.tree.as_ref().map(|tree| tree.root()) else {
        return;
    };

    let sidebar_fill = color(theme.chrome.sidebar);
    let accent = color(theme.accent);
    let selection_fill = chrome_text::sidebar_selection(theme);
    let size = chrome_text::SIDEBAR_PT * scale_factor;
    let mut pen = ChromePen::new(
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        font,
        glyph_instances,
    );

    // 1. Background and the separator on the right.
    quad_instances.push(QuadInstance::solid(
        [0.0, tab_bar_height, sidebar_width, content_height],
        sidebar_fill,
    ));
    quad_instances.push(QuadInstance::solid(
        [
            sidebar_width - stroke,
            tab_bar_height,
            stroke,
            content_height,
        ],
        color(theme.chrome.border),
    ));

    // 2. Header: the folder name, small caps style.
    let header_h = SIDEBAR_HEADER_HEIGHT * scale_factor;
    let header_mid = tab_bar_height + header_h * 0.5;
    if drop_target == Some(root_id) {
        quad_instances.push(QuadInstance::solid(
            [
                0.0,
                tab_bar_height + header_h - 2.0 * stroke,
                sidebar_width,
                2.0 * stroke,
            ],
            accent,
        ));
    }
    let header_ink = chrome_text::sidebar_header(theme);
    let header_size = chrome_text::HEADER_PT * scale_factor;
    pen.draw_tracked(
        header_size,
        chrome_text::HEADER_TRACKING_EM * header_size,
        &root_title,
        ROW_LEFT * scale_factor,
        header_mid,
        header_ink,
    );
    pen.draw(
        size,
        "×",
        sidebar_width - stroke - 18.0 * scale_factor,
        header_mid,
        header_ink,
    );

    // 3. Rows. The header above stays put. Rows are scissored to the
    // list, so a scrolled name cannot cover the title or the status bar.
    let rows_top = tab_bar_height + header_h;
    let list_bottom = tab_bar_height + content_height;
    let tree_bounds = [
        0.0,
        rows_top,
        (sidebar_width - stroke).max(0.0),
        (list_bottom - rows_top).max(0.0),
    ];
    let row_quads = quad_instances.len();
    let row_glyphs = pen.glyphs.len();
    let row_h = SIDEBAR_ROW_HEIGHT * scale_factor;

    let rows = crate::sidebar::folders::slots(sidebar);
    let selected_id = sidebar.selected_node;
    let editing_slot = sidebar
        .inline_input
        .as_ref()
        .and_then(|input| crate::sidebar::inline::slot_index(input, &rows));

    for (i, row) in rows.iter().enumerate() {
        let row_y = rows_top + i as f32 * row_h - sidebar.scroll_y;
        if row_y + row_h < rows_top || row_y > list_bottom {
            continue;
        }
        let mid = row_y + row_h * 0.5;

        let is_selected = match selected_id {
            Some(id) => id == row.id && row.root == 0,
            None => sidebar.followed_extra == Some((row.root, row.id)) && !row.title,
        };
        let row_fill = if is_selected {
            quad_instances.push(QuadInstance::solid(
                [0.0, row_y, sidebar_width - stroke, row_h],
                selection_fill,
            ));
            if sidebar.focused {
                quad_instances.push(QuadInstance::solid(
                    [0.0, row_y, 2.0 * stroke, row_h],
                    accent,
                ));
            }
            selection_fill
        } else {
            sidebar_fill
        };
        if drop_target == Some(row.id) && row.root == 0 {
            quad_instances.push(QuadInstance::solid(
                [
                    0.0,
                    row_y + row_h - 2.0 * stroke,
                    sidebar_width - stroke,
                    2.0 * stroke,
                ],
                accent,
            ));
        }

        let geometry = row_geometry(row.depth, scale_factor);
        let git = (row.root == 0).then(|| git_mark(sidebar, row.id)).flatten();
        let label_ink = match git {
            // Ignored paths are dimmed on purpose; they keep the muted ink.
            Some(mark @ crate::git::marks::GitMark::Ignored) => {
                color(crate::git::colors::mark_color(mark, theme))
            }
            Some(mark) => chrome_text::sidebar_label(
                color(crate::git::colors::mark_color(mark, theme)),
                row_fill,
            ),
            None => chrome_text::sidebar_label(color(theme.foreground), row_fill),
        };
        let outline_ink = chrome_text::outline_ink(theme, row_fill);
        let icon = ICON_PT * scale_factor;
        let icon_y = mid - icon * 0.5;
        if row.kind == NodeKind::Directory {
            let chevron = CHEVRON_PT * scale_factor;
            super::icons::push_disclosure(
                quad_instances,
                geometry.chevron_x,
                mid - chevron * 0.5,
                chevron,
                chrome_text::chevron_ink(
                    chrome_text::sidebar_label(color(theme.foreground), row_fill),
                    row_fill,
                ),
                row.expanded,
                stroke,
            );
            super::icons::push_folder_outline(
                quad_instances,
                geometry.icon_x,
                icon_y,
                icon,
                outline_ink,
                stroke,
            );
        } else {
            super::icons::push_document_outline(
                quad_instances,
                geometry.icon_x,
                icon_y,
                icon,
                outline_ink,
                stroke,
            );
        }

        let name_x = geometry.name_x;
        let editing = sidebar
            .inline_input
            .as_ref()
            .filter(|_| editing_slot == Some(i));
        if let Some(input) = editing {
            let input_text = input.field.as_str();
            let input_box_w = sidebar_width - stroke - name_x - 8.0 * scale_factor;
            quad_instances.push(QuadInstance::solid(
                [
                    name_x - 2.0 * stroke,
                    row_y + 2.0 * stroke,
                    input_box_w,
                    row_h - 4.0 * stroke,
                ],
                color(theme.chrome.panel_row),
            ));
            quad_instances.push(QuadInstance::solid(
                [
                    name_x - 2.0 * stroke,
                    row_y + row_h - 2.0 * stroke,
                    input_box_w,
                    stroke,
                ],
                accent,
            ));
            // Caret and selection edges measured on the text before them.
            let mut x_at = |chars: usize| {
                let end = input_text
                    .char_indices()
                    .nth(chars)
                    .map_or(input_text.len(), |(byte, _)| byte);
                name_x + pen.width(size, 0.0, &input_text[..end])
            };
            let (sel_start, sel_end) = input.field.selection();
            let caret_x = x_at(input.field.caret());
            if sel_start != sel_end {
                let left = x_at(sel_start);
                let right = x_at(sel_end);
                quad_instances.push(QuadInstance::solid(
                    [
                        left,
                        row_y + 3.0 * stroke,
                        right - left,
                        row_h - 6.0 * stroke,
                    ],
                    color(theme.selection),
                ));
            }
            let field_ink =
                chrome_text::sidebar_label(color(theme.foreground), color(theme.chrome.panel_row));
            pen.draw(size, input_text, name_x, mid, field_ink);
            quad_instances.push(QuadInstance::solid(
                [
                    caret_x,
                    row_y + 4.0 * stroke,
                    2.0 * stroke,
                    row_h - 8.0 * stroke,
                ],
                accent,
            ));
        } else {
            pen.draw(size, &row.name, name_x, mid, label_ink);
            let mark_x = sidebar_width - stroke - 18.0 * scale_factor;
            if row.title {
                let ink = chrome_text::sidebar_label(color(theme.muted), row_fill);
                pen.draw(size, "×", mark_x, mid, ink);
            } else if let Some(letter) = git.and_then(|mark| mark.letter()) {
                pen.draw(size, &letter.to_string(), mark_x, mid, label_ink);
            }
        }
    }
    super::clip::clip_quads(quad_instances, row_quads, tree_bounds);
    super::clip::clip_glyphs(pen.glyphs, row_glyphs, tree_bounds);

    if let Some(menu) = &sidebar.context_menu {
        let [mx, my, mw, mh] = menu.rect;
        quad_instances.push(QuadInstance::solid(
            [
                mx - stroke,
                my - stroke,
                mw + 2.0 * stroke,
                mh + 2.0 * stroke,
            ],
            color(theme.chrome.panel),
        ));
        let menu_fill = color(theme.chrome.panel_input);
        quad_instances.push(QuadInstance::solid([mx, my, mw, mh], menu_fill));
        let ink = chrome_text::sidebar_label(color(theme.foreground), menu_fill);
        let item_h = crate::sidebar::menu::SIDEBAR_MENU_ITEM_HEIGHT * scale_factor;
        for (idx, item) in menu.items.iter().enumerate() {
            let item_y = my + 4.0 * scale_factor + idx as f32 * item_h;
            pen.draw(
                size,
                item.label(),
                mx + 10.0 * stroke,
                item_y + item_h * 0.5,
                ink,
            );
        }
    }
}

/// Left edge of a depth-0 row's chevron, in points.
pub(crate) const ROW_LEFT: f32 = 8.0;
/// Indent per nesting level, in points.
pub(crate) const INDENT_PT: f32 = 16.0;
/// Disclosure triangle width, in points.
pub(crate) const CHEVRON_PT: f32 = 8.0;
/// Folder and document outline slot, in points.
pub(crate) const ICON_PT: f32 = 14.0;
const CHEVRON_SLOT_PT: f32 = 12.0;
const ICON_GAP_PT: f32 = 6.0;

/// Horizontal placement of one row's marks and name, in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RowGeometry {
    pub chevron_x: f32,
    pub icon_x: f32,
    pub name_x: f32,
}

/// Every row at `depth` puts its marks and name in the same columns, so
/// file names line up with folder names at the same depth.
pub(crate) fn row_geometry(depth: usize, scale: f32) -> RowGeometry {
    let left = (ROW_LEFT + depth as f32 * INDENT_PT) * scale;
    let chevron_x = left + (CHEVRON_SLOT_PT - CHEVRON_PT) * 0.5 * scale;
    let icon_x = left + CHEVRON_SLOT_PT * scale;
    RowGeometry {
        chevron_x,
        icon_x,
        name_x: icon_x + (ICON_PT + ICON_GAP_PT) * scale,
    }
}

fn git_mark(
    sidebar: &SidebarState,
    id: lux_ui::file_tree::NodeId,
) -> Option<crate::git::marks::GitMark> {
    let path = sidebar.tree.as_ref()?.node_path(id)?;
    sidebar.git_marks.mark_for(path)
}
