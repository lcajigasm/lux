//! Rendering logic for the file tree sidebar (G12).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::render::paint::{color, push_glyph};
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
    font_size: f32,
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

    let Some(tree) = sidebar.tree.as_mut() else {
        return;
    };

    let foreground = color(theme.foreground);
    let muted = color(theme.muted);
    let accent = color(theme.accent);

    let sidebar_font_size = font_size * 0.80;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size: sidebar_font_size,
        ligatures: false,
    };

    // 1. Sidebar Background
    quad_instances.push(QuadInstance {
        rect: [0.0, tab_bar_height, sidebar_width, content_height],
        color: [0.07, 0.07, 0.09, 1.0],
    });

    // 2. Right Separator Border
    let border_color = if sidebar.focused {
        accent
    } else {
        [0.18, 0.18, 0.22, 1.0]
    };
    quad_instances.push(QuadInstance {
        rect: [
            sidebar_width - stroke,
            tab_bar_height,
            stroke,
            content_height,
        ],
        color: border_color,
    });

    // 3. Header: Project Root
    let header_h = SIDEBAR_HEADER_HEIGHT * scale_factor;
    quad_instances.push(QuadInstance {
        rect: [0.0, tab_bar_height, sidebar_width - stroke, header_h],
        color: [0.06, 0.06, 0.08, 1.0],
    });
    quad_instances.push(QuadInstance {
        rect: [
            0.0,
            tab_bar_height + header_h - stroke,
            sidebar_width,
            stroke,
        ],
        color: [0.15, 0.15, 0.18, 1.0],
    });

    let header_line = editor_view
        .text_layout_mut()
        .layout_line(&style, &root_title, &[]);
    let header_x = 12.0 * scale_factor;
    let header_y = tab_bar_height + (header_h - sidebar_font_size) * 0.5;
    for c in header_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            sidebar_font_size,
            &c,
            header_x,
            header_y + header_line.baseline(),
            muted,
            glyph_instances,
        );
    }

    // 4. File Tree Rows
    let rows_top = tab_bar_height + header_h;
    let row_h = SIDEBAR_ROW_HEIGHT * scale_factor;
    let baseline = (row_h - sidebar_font_size) * 0.5;

    let rows = tree.rows().to_vec();
    let selected_id = sidebar.selected_node;

    for (i, row) in rows.iter().enumerate() {
        let row_y = rows_top + i as f32 * row_h - sidebar.scroll_y;
        if row_y + row_h < rows_top || row_y > tab_bar_height + content_height {
            continue;
        }

        let is_selected = selected_id == Some(row.id);
        if is_selected {
            quad_instances.push(QuadInstance {
                rect: [0.0, row_y, sidebar_width - stroke, row_h],
                color: [0.16, 0.20, 0.28, 1.0],
            });
            quad_instances.push(QuadInstance {
                rect: [0.0, row_y, 2.0 * stroke, row_h],
                color: accent,
            });
        }

        let indent = 12.0 * scale_factor + (row.depth as f32 * 12.0 * scale_factor);

        // Icon/Arrow indicator
        let icon = match row.kind {
            NodeKind::Directory => {
                if row.expanded {
                    "v"
                } else {
                    ">"
                }
            }
            NodeKind::File => "-",
        };
        let icon_line = editor_view.text_layout_mut().layout_line(&style, icon, &[]);
        let icon_color = match row.kind {
            NodeKind::Directory => accent,
            NodeKind::File => muted,
        };
        for c in icon_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                sidebar_font_size,
                &c,
                indent,
                row_y + baseline + icon_line.baseline(),
                icon_color,
                glyph_instances,
            );
        }

        // Node Name or Inline Input
        let name_x = indent + 14.0 * scale_factor;
        let is_editing = sidebar
            .inline_input
            .as_ref()
            .is_some_and(|inp| inp.row_idx == i);

        if is_editing {
            let input_text = sidebar
                .inline_input
                .as_ref()
                .map(|inp| inp.text.as_str())
                .unwrap_or("");
            let input_box_w = sidebar_width - stroke - name_x - 8.0 * scale_factor;
            quad_instances.push(QuadInstance {
                rect: [
                    name_x - 2.0 * stroke,
                    row_y + 2.0 * stroke,
                    input_box_w,
                    row_h - 4.0 * stroke,
                ],
                color: [0.10, 0.12, 0.18, 1.0],
            });
            quad_instances.push(QuadInstance {
                rect: [
                    name_x - 2.0 * stroke,
                    row_y + row_h - 2.0 * stroke,
                    input_box_w,
                    stroke,
                ],
                color: accent,
            });
            let edit_line = editor_view
                .text_layout_mut()
                .layout_line(&style, input_text, &[]);
            for c in edit_line.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    sidebar_font_size,
                    &c,
                    name_x,
                    row_y + baseline + edit_line.baseline(),
                    foreground,
                    glyph_instances,
                );
            }
            let caret_x = name_x + edit_line.width();
            quad_instances.push(QuadInstance {
                rect: [
                    caret_x,
                    row_y + 4.0 * stroke,
                    2.0 * stroke,
                    row_h - 8.0 * stroke,
                ],
                color: accent,
            });
        } else {
            let name_line = editor_view
                .text_layout_mut()
                .layout_line(&style, &row.name, &[]);
            let text_color = if is_selected || row.kind == NodeKind::Directory {
                foreground
            } else {
                muted
            };
            for c in name_line.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    sidebar_font_size,
                    &c,
                    name_x,
                    row_y + baseline + name_line.baseline(),
                    text_color,
                    glyph_instances,
                );
            }
        }
    }

    if let Some(menu) = &sidebar.context_menu {
        let [mx, my, mw, mh] = menu.rect;
        quad_instances.push(QuadInstance {
            rect: [
                mx - stroke,
                my - stroke,
                mw + 2.0 * stroke,
                mh + 2.0 * stroke,
            ],
            color: [0.06, 0.06, 0.10, 0.98],
        });
        quad_instances.push(QuadInstance {
            rect: [mx, my, mw, mh],
            color: [0.14, 0.14, 0.20, 1.0],
        });
        let item_h = crate::sidebar::menu::SIDEBAR_MENU_ITEM_HEIGHT * scale_factor;
        for (idx, item) in menu.items.iter().enumerate() {
            let item_y = my + 4.0 * scale_factor + idx as f32 * item_h;
            let label = item.label();
            let layout = editor_view
                .text_layout_mut()
                .layout_line(&style, label, &[]);
            let text_y = item_y + (item_h - sidebar_font_size) * 0.5;
            for cluster in layout.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    sidebar_font_size,
                    &cluster,
                    mx + 10.0 * stroke,
                    text_y + layout.baseline(),
                    foreground,
                    glyph_instances,
                );
            }
        }
    }
}
