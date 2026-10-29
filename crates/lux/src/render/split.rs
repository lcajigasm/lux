//! Split editor multi-panel rendering (ST-SPLIT-01).

use lux_core::bookmarks::BookmarkState;
use lux_core::buffer::Buffer;
use lux_core::folding::FoldState;
use lux_core::multicursor::MultiCursor;
use lux_render::{GlyphInstance, QuadInstance};
use lux_ui::{EditorView, Theme};
use swash::FontRef;

use crate::app::{AtlasState, BLINK_INTERVAL, SyntaxState};
use crate::document::Documents;
use crate::pane_view::PaneView;
use crate::render::paint::{color, push_glyph, syntax_color};
use crate::split::{SplitLayout, compute_layout_geometry};

/// Strictly clamps a quad instance within a bounding rectangle.
/// Returns None if the quad falls completely outside the bounds.
pub(crate) fn clamp_quad_to_rect(rect: [f32; 4], bounds: [f32; 4]) -> Option<[f32; 4]> {
    let [rx, ry, rw, rh] = rect;
    let [bx, by, bw, bh] = bounds;
    let x1 = rx.max(bx);
    let y1 = ry.max(by);
    let x2 = (rx + rw).min(bx + bw);
    let y2 = (ry + rh).min(by + bh);
    if x2 > x1 && y2 > y1 {
        Some([x1, y1, x2 - x1, y2 - y1])
    } else {
        None
    }
}

/// Parameters required to render the full multi-pane split view (ST-SPLIT-01).
pub(crate) struct SplitRenderParams<'a> {
    pub(crate) split: &'a SplitLayout,
    pub(crate) documents: &'a Documents,
    pub(crate) primary_rect: [f32; 4],
    pub(crate) scale_factor: f32,
    pub(crate) stroke: f32,
    pub(crate) font_size: f32,
    pub(crate) line_height: f32,
    pub(crate) font: FontRef<'static>,
    pub(crate) active_syntax: Option<&'a SyntaxState>,
    pub(crate) active_buffer: &'a Buffer,
    pub(crate) active_cursors: &'a MultiCursor,
    pub(crate) active_fold_state: &'a FoldState,
    pub(crate) active_bookmarks: &'a BookmarkState,
    pub(crate) elapsed: std::time::Duration,
    pub(crate) active_scroll: (f32, f32),
    pub(crate) theme: &'a Theme,
    pub(crate) syntax_theme: &'static lux_syntax::theme::SyntaxTheme,
}

pub(crate) fn render_split_layout(
    params: &SplitRenderParams,
    editor_view: &mut EditorView,
    atlas_state: &mut AtlasState,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    if !params.split.is_active() {
        return;
    }

    let geom = compute_layout_geometry(params.split, params.primary_rect, params.stroke);

    // 1. Render all dividers between panes
    for (rect, div_type) in &geom.dividers {
        let is_dragging = params.split.dragging_divider == Some(*div_type);
        let divider_color = if is_dragging {
            color(params.theme.accent)
        } else {
            [0.20, 0.20, 0.28, 1.0]
        };
        quad_instances.push(QuadInstance {
            rect: *rect,
            color: divider_color,
        });
    }

    // 2. Render each pane view with strict geometric boundaries
    let fallback_tab = params
        .documents
        .active_tab
        .or_else(|| params.documents.tabs.tabs().first().map(|t| t.id));
    let Some(fallback_tab) = fallback_tab else {
        return;
    };

    for (i, &pane_rect) in geom.pane_rects.iter().enumerate() {
        let default_pane = PaneView::with_geometry(fallback_tab, pane_rect);
        let pane = params.split.panes.get(i).unwrap_or(&default_pane);
        let is_active_pane = i == params.split.active_pane;

        render_single_pane(
            pane,
            pane_rect,
            is_active_pane,
            params,
            editor_view,
            atlas_state,
            quad_instances,
            glyph_instances,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_single_pane(
    pane: &PaneView,
    bounds: [f32; 4],
    is_active_pane: bool,
    params: &SplitRenderParams,
    editor_view: &mut EditorView,
    atlas_state: &mut AtlasState,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) {
    let [px, py, pw, ph] = bounds;

    // Pane background
    quad_instances.push(QuadInstance {
        rect: bounds,
        color: color(params.theme.background),
    });

    // Active pane indicator border
    if is_active_pane
        && let Some(top_border) = clamp_quad_to_rect([px, py, pw, 2.0 * params.stroke], bounds)
    {
        quad_instances.push(QuadInstance {
            rect: top_border,
            color: color(params.theme.accent),
        });
    }

    // Resolve buffer, cursors, syntax tokens, fold state, and bookmarks
    let entry_match = params
        .documents
        .entries
        .iter()
        .find(|e| e.tab_id == pane.tab_id);

    let active_bid = params.documents.active_buffer_id();
    let is_cloned_active_buffer = if is_active_pane {
        true
    } else if let Some(active_id) = params.documents.active_tab
        && pane.tab_id == active_id
    {
        true
    } else if let (Some(active_b), Some(entry)) = (active_bid, entry_match) {
        entry.buffer_id == active_b
    } else {
        false
    };

    let (buffer, cursors, fold_state, bookmarks, tokens): (
        &Buffer,
        &MultiCursor,
        &FoldState,
        &BookmarkState,
        &[lux_syntax::Token],
    ) = if is_active_pane {
        (
            params.active_buffer,
            params.active_cursors,
            params.active_fold_state,
            params.active_bookmarks,
            params
                .active_syntax
                .map(|s| s.tokens.as_slice())
                .unwrap_or(&[]),
        )
    } else if is_cloned_active_buffer {
        let tok_slice = if let Some(syntax) = params.active_syntax {
            syntax.tokens.as_slice()
        } else if !pane.tokens.is_empty() {
            pane.tokens.as_slice()
        } else {
            &[]
        };
        (
            params.active_buffer,
            &pane.cursors,
            &pane.fold_state,
            &pane.bookmarks,
            tok_slice,
        )
    } else if let Some(entry) = entry_match {
        let tok_slice = if !pane.tokens.is_empty() {
            pane.tokens.as_slice()
        } else {
            &[]
        };
        (
            &entry.buffer,
            &pane.cursors,
            &pane.fold_state,
            &pane.bookmarks,
            tok_slice,
        )
    } else {
        (
            params.active_buffer,
            &pane.cursors,
            &pane.fold_state,
            &pane.bookmarks,
            &[],
        )
    };

    let (scroll_x, scroll_y) = if is_active_pane {
        params.active_scroll
    } else {
        (pane.viewport.scroll_x, pane.viewport.scroll_y)
    };
    let mut pane_viewport = pane.viewport.to_editor_viewport();
    pane_viewport.scroll_x = scroll_x;
    pane_viewport.scroll_y = scroll_y;
    let frame = editor_view.build_frame(
        buffer,
        cursors,
        &pane_viewport,
        params.elapsed,
        Some(fold_state),
    );

    let gutter_total_width = frame.gutter_width + 16.0 * params.scale_factor;
    let text_left = px + gutter_total_width + 6.0 * params.scale_factor;

    // Gutter background & right border
    if gutter_total_width > 0.0 {
        if let Some(g_rect) = clamp_quad_to_rect([px, py, gutter_total_width, ph], bounds) {
            quad_instances.push(QuadInstance {
                rect: g_rect,
                color: [0.10, 0.10, 0.13, 1.0],
            });
        }
        if let Some(b_rect) = clamp_quad_to_rect(
            [
                px + gutter_total_width - params.stroke,
                py,
                params.stroke,
                ph,
            ],
            bounds,
        ) {
            quad_instances.push(QuadInstance {
                rect: b_rect,
                color: [0.18, 0.18, 0.22, 1.0],
            });
        }
    }

    let text_bounds = [
        px + gutter_total_width,
        py,
        (pw - gutter_total_width).max(0.0),
        ph,
    ];

    // Active line highlight
    for cursor in &frame.cursors {
        if cursor.primary {
            let cur_y = py + cursor.visual_row as f32 * params.line_height - scroll_y;
            if let Some(h_rect) = clamp_quad_to_rect(
                [
                    px + gutter_total_width,
                    cur_y,
                    pw - gutter_total_width,
                    params.line_height,
                ],
                text_bounds,
            ) {
                quad_instances.push(QuadInstance {
                    rect: h_rect,
                    color: [0.13, 0.13, 0.17, 0.6],
                });
            }
        }
    }

    // Column rulers
    let muted_col = color(params.theme.muted);
    for ruler_x in &frame.column_ruler_x {
        let rx = text_left + ruler_x - scroll_x;
        if let Some(r_rect) = clamp_quad_to_rect([rx, py, params.stroke, ph], text_bounds) {
            quad_instances.push(QuadInstance {
                rect: r_rect,
                color: muted_col,
            });
        }
    }

    // Indent guides and selections
    for line in &frame.lines {
        let y = py + line.visual_index as f32 * params.line_height - scroll_y;
        for guide_x in &line.indent_guides {
            let gx = text_left + guide_x - scroll_x;
            if let Some(g_rect) =
                clamp_quad_to_rect([gx, y, params.stroke, params.line_height], text_bounds)
            {
                quad_instances.push(QuadInstance {
                    rect: g_rect,
                    color: muted_col,
                });
            }
        }
    }

    let sel_col = color(params.theme.selection);
    for selection in &frame.selections {
        let y = py + selection.visual_row as f32 * params.line_height - scroll_y;
        let width = (selection.x_end - selection.x_start).max(1.0);
        let sx = text_left + selection.x_start - scroll_x;
        if let Some(s_rect) = clamp_quad_to_rect([sx, y, width, params.line_height], text_bounds) {
            quad_instances.push(QuadInstance {
                rect: s_rect,
                color: sel_col,
            });
        }
    }

    let foreground_col = color(params.theme.foreground);
    let line_pad_y = (params.line_height - params.font_size) * 0.5;

    // Gutter numbers, fold indicators, and syntax-highlighted text clusters
    for line in &frame.lines {
        let y = py + line.visual_index as f32 * params.line_height - scroll_y;
        if y + params.line_height < py || y > py + ph {
            continue;
        }

        // Bookmark ribbon in gutter
        if bookmarks.contains(line.index) {
            let bm_x = px + 2.0 * params.stroke;
            let bm_w = 4.0 * params.stroke;
            let bm_h = (params.line_height * 0.60).clamp(8.0 * params.stroke, 14.0 * params.stroke);
            let bm_y = y + (params.line_height - bm_h) / 2.0;
            if let Some(bm_rect) = clamp_quad_to_rect([bm_x, bm_y, bm_w, bm_h], bounds) {
                quad_instances.push(QuadInstance {
                    rect: bm_rect,
                    color: [0.25, 0.70, 0.95, 0.95],
                });
            }
        }

        // Line numbers
        let is_curr = frame.cursors.iter().any(|c| c.line == line.index);
        let num_color = if is_curr { foreground_col } else { muted_col };
        for cluster in line.number_layout.clusters() {
            let nx = px + 8.0 * params.scale_factor + cluster.x;
            if nx >= px && nx + cluster.advance <= px + gutter_total_width {
                push_glyph(
                    &mut atlas_state.atlas,
                    &mut atlas_state.dirty,
                    &mut atlas_state.scale_cx,
                    params.font,
                    params.font_size,
                    &cluster,
                    px + 8.0 * params.scale_factor,
                    y + line_pad_y + line.number_layout.baseline(),
                    num_color,
                    glyph_instances,
                );
            }
        }

        // Syntax-colored text clusters bounded to pane geometry
        let line_start = buffer.line_start_byte(line.index).unwrap_or(0);
        let line_len = buffer.line_content(line.index).map_or(0, |c| c.len());
        let line_end = line_start + line_len;
        let line_tokens: &[lux_syntax::Token] = if !tokens.is_empty() {
            let start_idx = tokens.partition_point(|t| t.end_byte <= line_start);
            let end_idx =
                tokens[start_idx..].partition_point(|t| t.start_byte < line_end) + start_idx;
            &tokens[start_idx..end_idx]
        } else {
            &[]
        };

        for cluster in line.layout.clusters() {
            let cx = text_left + cluster.x - scroll_x;
            if cx < text_left || cx + cluster.advance > px + pw {
                continue;
            }
            let abs_byte = line_start + cluster.byte_range.start;
            let glyph_color = if let Some(token) = line_tokens
                .iter()
                .find(|t| t.start_byte <= abs_byte && abs_byte < t.end_byte)
            {
                syntax_color(params.syntax_theme.color_for_token(token))
            } else {
                foreground_col
            };

            push_glyph(
                &mut atlas_state.atlas,
                &mut atlas_state.dirty,
                &mut atlas_state.scale_cx,
                params.font,
                params.font_size,
                &cluster,
                text_left - scroll_x,
                y + line_pad_y + line.layout.baseline(),
                glyph_color,
                glyph_instances,
            );
        }
    }

    // Caret indicators (blinking in active pane, static solid in inactive pane)
    let caret_h = params.font_size + 2.0 * params.stroke;
    let blink_visible = (params.elapsed.as_millis() / BLINK_INTERVAL.as_millis()).is_multiple_of(2);

    for cursor_glyph in &frame.cursors {
        let x = text_left + cursor_glyph.x - scroll_x;
        let y = py + cursor_glyph.visual_row as f32 * params.line_height - scroll_y + line_pad_y
            - params.stroke;

        let should_draw = if is_active_pane {
            cursor_glyph.visible && blink_visible
        } else {
            true // static caret in inactive pane
        };

        if should_draw {
            let caret_color = if is_active_pane {
                color(params.theme.accent)
            } else {
                let [r, g, b, _] = color(params.theme.accent);
                [r, g, b, 0.65]
            };
            if let Some(c_rect) =
                clamp_quad_to_rect([x, y, 2.0 * params.stroke, caret_h], text_bounds)
            {
                quad_instances.push(QuadInstance {
                    rect: c_rect,
                    color: caret_color,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clamp_quad_to_rect_inside_and_overflow() {
        let bounds = [100.0, 50.0, 200.0, 300.0];

        // 1. Fully inside
        let inside = [120.0, 60.0, 50.0, 40.0];
        assert_eq!(clamp_quad_to_rect(inside, bounds), Some(inside));

        // 2. Overflowing left and top
        let over_left = [80.0, 40.0, 50.0, 40.0];
        let clamped = clamp_quad_to_rect(over_left, bounds).unwrap();
        assert_eq!(clamped, [100.0, 50.0, 30.0, 30.0]);

        // 3. Overflowing right and bottom
        let over_right = [280.0, 330.0, 50.0, 40.0];
        let clamped2 = clamp_quad_to_rect(over_right, bounds).unwrap();
        assert_eq!(clamped2, [280.0, 330.0, 20.0, 20.0]);

        // 4. Completely outside
        let outside = [400.0, 500.0, 50.0, 40.0];
        assert_eq!(clamp_quad_to_rect(outside, bounds), None);
    }

    #[test]
    fn test_render_split_layout_bounds_and_clipping() {
        use crate::app::App;
        use crate::split::LayoutPreset;

        let mut app =
            App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
        app.insert_at_cursor("let x = 42;\nfn hello() -> i32 {\n    x + 1\n}\n");

        // Split editor into 2 columns
        app.set_layout(LayoutPreset::Columns2);
        assert!(app.ui.split.is_active());
        assert_eq!(app.ui.split.panes.len(), 2);

        let p0 = &app.ui.split.panes[0];
        let p1 = &app.ui.split.panes[1];
        assert!(p0.viewport.width > 0.0);
        assert!(p1.viewport.width > 0.0);
        assert!(p1.viewport.x > p0.viewport.x);

        let mut quad_instances = Vec::new();
        let mut glyph_instances = Vec::new();

        let params = SplitRenderParams {
            split: &app.ui.split,
            documents: &app.documents,
            primary_rect: [0.0, 0.0, 800.0, 600.0],
            scale_factor: 1.0,
            stroke: 1.0,
            font_size: 14.0,
            line_height: 20.0,
            font: app.font,
            active_syntax: app.syntax.as_ref(),
            active_buffer: &app.buffer,
            active_cursors: &app.cursors,
            active_fold_state: &app.fold_state,
            active_bookmarks: &app.bookmarks,
            elapsed: std::time::Duration::from_millis(100),
            active_scroll: (0.0, 0.0),
            theme: &app.chrome.theme,
            syntax_theme: lux_syntax::theme::by_name(app.chrome.theme.name)
                .unwrap_or(&lux_syntax::theme::LUX_DARK),
        };

        render_split_layout(
            &params,
            &mut app.editor_view,
            &mut app.atlas_state,
            &mut quad_instances,
            &mut glyph_instances,
        );

        // Verify quads and glyphs were generated
        assert!(
            !quad_instances.is_empty(),
            "quads must be rendered for split layout"
        );
        assert!(
            !glyph_instances.is_empty(),
            "glyphs must be rendered for split layout"
        );

        // Geometric verification: no quad or glyph in pane 0 spills into pane 1
        let geom = compute_layout_geometry(&app.ui.split, [0.0, 0.0, 800.0, 600.0], 1.0);
        let p0_bounds = geom.pane_rects[0];
        let p1_bounds = geom.pane_rects[1];
        let div_x = geom.dividers[0].0[0];

        for quad in &quad_instances {
            let [qx, _qy, qw, _qh] = quad.rect;
            // Skip the divider itself
            if (qx - div_x).abs() < 1.0 {
                continue;
            }
            if qx < div_x {
                assert!(qx >= p0_bounds[0] - 0.01, "qx {qx} < p0 left");
                assert!(
                    qx + qw <= p0_bounds[0] + p0_bounds[2] + 0.01,
                    "pane 0 quad at {qx} width {qw} overflows p0 right {}",
                    p0_bounds[0] + p0_bounds[2]
                );
            } else {
                assert!(qx >= p1_bounds[0] - 0.01, "qx {qx} < p1 left");
                assert!(
                    qx + qw <= p1_bounds[0] + p1_bounds[2] + 0.01,
                    "pane 1 quad at {qx} width {qw} overflows p1 right {}",
                    p1_bounds[0] + p1_bounds[2]
                );
            }
        }

        for glyph in &glyph_instances {
            let [gx, _gy, gw, _gh] = glyph.rect;
            if gx < div_x {
                assert!(gx >= p0_bounds[0] - 0.01, "gx {gx} < p0 left");
                assert!(
                    gx + gw <= p0_bounds[0] + p0_bounds[2] + 0.01,
                    "pane 0 glyph at {gx} width {gw} overflows p0 right {}",
                    p0_bounds[0] + p0_bounds[2]
                );
            } else {
                assert!(gx >= p1_bounds[0] - 0.01, "gx {gx} < p1 left");
                assert!(
                    gx + gw <= p1_bounds[0] + p1_bounds[2] + 0.01,
                    "pane 1 glyph at {gx} width {gw} overflows p1 right {}",
                    p1_bounds[0] + p1_bounds[2]
                );
            }
        }
    }
}
