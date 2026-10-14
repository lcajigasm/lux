//! Rendering for in-process Git integration (G15).
//! Gutter indicators, interactive hunk popup, and conflict decorations.

use lux_git::gutter::{GutterMarker, HunkKind};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;
use swash::FontRef;
use swash::scale::ScaleContext;

use crate::git::{ConflictMarkerBlock, GutterHunkPopup};
use crate::render::paint::push_glyph;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_git(
    git: &mut crate::git::GitState,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef,
    sidebar_width: f32,
    gutter_width: f32,
    viewport_width: f32,
    viewport_height: f32,
    editor_top: f32,
    line_height: f32,
    scroll_y: f32,
    scale_factor: f32,
) {
    render_gutter_markers(
        &git.markers,
        sidebar_width,
        gutter_width,
        editor_top,
        line_height,
        scroll_y,
        scale_factor,
        viewport_height,
        quad_instances,
    );
    render_conflict_decorations(
        &mut git.conflict_blocks,
        quad_instances,
        glyph_instances,
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        font,
        sidebar_width,
        viewport_width,
        editor_top,
        line_height,
        scroll_y,
        scale_factor,
    );
    if let Some(popup) = &mut git.active_popup {
        render_hunk_popup(
            popup,
            quad_instances,
            glyph_instances,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            sidebar_width,
            gutter_width,
            editor_top,
            line_height,
            scroll_y,
            scale_factor,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_gutter_markers(
    markers: &[GutterMarker],
    sidebar_width: f32,
    gutter_width: f32,
    editor_top: f32,
    line_height: f32,
    scroll_y: f32,
    scale_factor: f32,
    viewport_height: f32,
    quad_instances: &mut Vec<QuadInstance>,
) {
    if markers.is_empty() {
        return;
    }
    let marker_w = 3.0 * scale_factor;
    let marker_x = sidebar_width + gutter_width + 1.0 * scale_factor;

    for marker in markers {
        let line_0 = marker.line.saturating_sub(1);
        let y = editor_top + line_0 as f32 * line_height - scroll_y;
        if y + line_height < editor_top || y > editor_top + viewport_height {
            continue;
        }

        let (color, h) = match marker.kind {
            HunkKind::Added => ([0.22, 0.72, 0.35, 0.95], line_height),
            HunkKind::Modified => ([0.25, 0.55, 0.92, 0.95], line_height),
            HunkKind::Removed => ([0.90, 0.25, 0.25, 0.95], 3.0 * scale_factor),
        };

        quad_instances.push(QuadInstance {
            rect: [marker_x, y, marker_w, h],
            color,
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_hunk_popup(
    popup: &mut GutterHunkPopup,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef,
    sidebar_width: f32,
    gutter_width: f32,
    editor_top: f32,
    line_height: f32,
    scroll_y: f32,
    scale_factor: f32,
) {
    let card_w = 260.0 * scale_factor;
    let card_h = 56.0 * scale_factor;
    let card_x = sidebar_width + gutter_width + 16.0 * scale_factor;
    let card_y = (editor_top + popup.line_index.saturating_sub(1) as f32 * line_height - scroll_y)
        .max(editor_top + 4.0);

    popup.rect = [card_x, card_y, card_w, card_h];

    // Background shadow / border
    quad_instances.push(QuadInstance {
        rect: [card_x - 1.0, card_y - 1.0, card_w + 2.0, card_h + 2.0],
        color: [0.3, 0.35, 0.45, 0.9],
    });
    quad_instances.push(QuadInstance {
        rect: [card_x, card_y, card_w, card_h],
        color: [0.12, 0.13, 0.17, 0.98],
    });

    let font_size = 11.0 * scale_factor;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };

    // Label
    let title = format!(
        "Hunk: -{} +{} lines",
        popup.hunk.before_len, popup.hunk.after_len
    );
    let title_line = editor_view
        .text_layout_mut()
        .layout_line(&style, &title, &[]);
    let title_x = card_x + 8.0 * scale_factor;
    let title_y = card_y + 6.0 * scale_factor;
    for cluster in title_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            title_x,
            title_y + title_line.baseline(),
            [0.85, 0.88, 0.95, 1.0],
            glyph_instances,
        );
    }

    // Action buttons: [Stage] [Revert] [Close]
    let btn_y = card_y + 28.0 * scale_factor;
    let btn_h = 20.0 * scale_factor;

    // Stage button
    let stage_w = 75.0 * scale_factor;
    let stage_x = card_x + 8.0 * scale_factor;
    popup.stage_btn = [stage_x, btn_y, stage_w, btn_h];
    quad_instances.push(QuadInstance {
        rect: [stage_x, btn_y, stage_w, btn_h],
        color: [0.18, 0.48, 0.28, 0.9],
    });
    let stage_text = editor_view
        .text_layout_mut()
        .layout_line(&style, "Stage Hunk", &[]);
    for cluster in stage_text.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            stage_x + 6.0 * scale_factor,
            btn_y + 2.0 * scale_factor + stage_text.baseline(),
            [0.95, 0.98, 0.95, 1.0],
            glyph_instances,
        );
    }

    // Revert button
    let rev_w = 60.0 * scale_factor;
    let rev_x = stage_x + stage_w + 6.0 * scale_factor;
    popup.revert_btn = [rev_x, btn_y, rev_w, btn_h];
    quad_instances.push(QuadInstance {
        rect: [rev_x, btn_y, rev_w, btn_h],
        color: [0.55, 0.22, 0.22, 0.9],
    });
    let rev_text = editor_view
        .text_layout_mut()
        .layout_line(&style, "Revert", &[]);
    for cluster in rev_text.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            rev_x + 6.0 * scale_factor,
            btn_y + 2.0 * scale_factor + rev_text.baseline(),
            [0.98, 0.90, 0.90, 1.0],
            glyph_instances,
        );
    }

    // Close button
    let close_w = 50.0 * scale_factor;
    let close_x = rev_x + rev_w + 6.0 * scale_factor;
    popup.close_btn = [close_x, btn_y, close_w, btn_h];
    quad_instances.push(QuadInstance {
        rect: [close_x, btn_y, close_w, btn_h],
        color: [0.25, 0.27, 0.32, 0.9],
    });
    let close_text = editor_view
        .text_layout_mut()
        .layout_line(&style, "Close", &[]);
    for cluster in close_text.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            close_x + 6.0 * scale_factor,
            btn_y + 2.0 * scale_factor + close_text.baseline(),
            [0.85, 0.88, 0.95, 1.0],
            glyph_instances,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_conflict_decorations(
    blocks: &mut [ConflictMarkerBlock],
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef,
    sidebar_width: f32,
    viewport_width: f32,
    editor_top: f32,
    line_height: f32,
    scroll_y: f32,
    scale_factor: f32,
) {
    if blocks.is_empty() {
        return;
    }

    let font_size = 11.0 * scale_factor;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };

    for block in blocks.iter_mut() {
        let start_y = editor_top + block.start_line as f32 * line_height - scroll_y;
        let sep_y = editor_top + block.sep_line as f32 * line_height - scroll_y;
        let end_y = editor_top + block.end_line as f32 * line_height - scroll_y;

        // Tint ours section (start_line to sep_line)
        if sep_y > start_y {
            quad_instances.push(QuadInstance {
                rect: [sidebar_width, start_y, viewport_width, sep_y - start_y],
                color: [0.15, 0.35, 0.20, 0.25],
            });
        }

        // Tint theirs section (sep_line to end_line)
        if end_y > sep_y {
            quad_instances.push(QuadInstance {
                rect: [sidebar_width, sep_y, viewport_width, end_y - sep_y],
                color: [0.15, 0.25, 0.40, 0.25],
            });
        }

        // CodeLens style buttons directly above start_line
        let btn_h = 18.0 * scale_factor;
        let btn_y = (start_y - btn_h - 2.0).max(editor_top);

        let ours_w = 95.0 * scale_factor;
        let ours_x = sidebar_width + 20.0 * scale_factor;
        block.accept_ours_rect = [ours_x, btn_y, ours_w, btn_h];
        quad_instances.push(QuadInstance {
            rect: [ours_x, btn_y, ours_w, btn_h],
            color: [0.18, 0.45, 0.25, 0.9],
        });
        let t_ours = editor_view
            .text_layout_mut()
            .layout_line(&style, "Accept Current", &[]);
        for c in t_ours.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                font_size,
                &c,
                ours_x + 4.0 * scale_factor,
                btn_y + 1.0 * scale_factor + t_ours.baseline(),
                [0.95, 0.98, 0.95, 1.0],
                glyph_instances,
            );
        }

        let theirs_w = 105.0 * scale_factor;
        let theirs_x = ours_x + ours_w + 6.0 * scale_factor;
        block.accept_theirs_rect = [theirs_x, btn_y, theirs_w, btn_h];
        quad_instances.push(QuadInstance {
            rect: [theirs_x, btn_y, theirs_w, btn_h],
            color: [0.18, 0.32, 0.55, 0.9],
        });
        let t_theirs = editor_view
            .text_layout_mut()
            .layout_line(&style, "Accept Incoming", &[]);
        for c in t_theirs.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                font_size,
                &c,
                theirs_x + 4.0 * scale_factor,
                btn_y + 1.0 * scale_factor + t_theirs.baseline(),
                [0.92, 0.95, 0.98, 1.0],
                glyph_instances,
            );
        }

        let both_w = 80.0 * scale_factor;
        let both_x = theirs_x + theirs_w + 6.0 * scale_factor;
        block.accept_both_rect = [both_x, btn_y, both_w, btn_h];
        quad_instances.push(QuadInstance {
            rect: [both_x, btn_y, both_w, btn_h],
            color: [0.35, 0.30, 0.45, 0.9],
        });
        let t_both = editor_view
            .text_layout_mut()
            .layout_line(&style, "Accept Both", &[]);
        for c in t_both.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                font_size,
                &c,
                both_x + 4.0 * scale_factor,
                btn_y + 1.0 * scale_factor + t_both.baseline(),
                [0.95, 0.92, 0.98, 1.0],
                glyph_instances,
            );
        }
    }
}
