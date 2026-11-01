//! Rendering for Language Server Protocol UI (Goal G16).
//! Diagnostics squiggles, autocompletion popup, hover tooltips, and problems panel.

use lux_lsp::async_lsp::lsp_types::{CompletionItemKind, DiagnosticSeverity, Url};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;
use swash::FontRef;
use swash::scale::ScaleContext;

use crate::lsp::{CompletionPopupState, DiagnosticItem, HoverPopupState, ProblemsPanelState};
use crate::render::paint::{active_theme, color, push_glyph};

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_lsp(
    lsp: &mut crate::lsp::LspState,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef,
    sidebar_width: f32,
    text_left: f32,
    viewport_width: f32,
    viewport_height: f32,
    editor_top: f32,
    line_height: f32,
    scroll_x: f32,
    scroll_y: f32,
    scale_factor: f32,
    cursor_x: f32,
    cursor_y: f32,
) {
    // 1. Diagnostics squiggles
    render_diagnostics_squiggles(
        &lsp.diagnostics,
        text_left,
        editor_top,
        line_height,
        scroll_x,
        scroll_y,
        scale_factor,
        viewport_height,
        quad_instances,
    );

    // 2. Completion popup
    if let Some(popup) = &mut lsp.completion_popup {
        render_completion_popup(
            popup,
            quad_instances,
            glyph_instances,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            cursor_x,
            cursor_y + line_height,
            scale_factor,
        );
    }

    // 3. Hover tooltip
    if let Some(hover) = &mut lsp.hover_popup {
        render_hover_popup(
            hover,
            quad_instances,
            glyph_instances,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            cursor_x,
            (cursor_y - 40.0 * scale_factor).max(editor_top + 4.0),
            scale_factor,
        );
    }

    // 4. Problems panel
    if lsp.problems.visible {
        render_problems_panel(
            &mut lsp.problems,
            &lsp.all_diagnostics,
            quad_instances,
            glyph_instances,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            sidebar_width,
            viewport_width,
            editor_top + viewport_height - 160.0 * scale_factor,
            160.0 * scale_factor,
            scale_factor,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_diagnostics_squiggles(
    diagnostics: &[DiagnosticItem],
    text_left: f32,
    editor_top: f32,
    line_height: f32,
    scroll_x: f32,
    scroll_y: f32,
    scale_factor: f32,
    viewport_height: f32,
    quad_instances: &mut Vec<QuadInstance>,
) {
    let char_w = 8.5 * scale_factor;
    for diag in diagnostics {
        let line = diag.range.start.line as usize;
        let y =
            editor_top + line as f32 * line_height - scroll_y + line_height - 2.0 * scale_factor;
        if y < editor_top || y > editor_top + viewport_height {
            continue;
        }

        let start_col = diag.range.start.character as f32;
        let end_col = (diag.range.end.character as f32).max(start_col + 1.0);
        let x_start = text_left + start_col * char_w - scroll_x;
        let x_end = text_left + end_col * char_w - scroll_x;
        let width = (x_end - x_start).max(char_w);

        let color = match diag.severity {
            Some(DiagnosticSeverity::ERROR) | None => [0.95, 0.25, 0.25, 0.95],
            Some(DiagnosticSeverity::WARNING) => [0.90, 0.68, 0.15, 0.95],
            Some(DiagnosticSeverity::INFORMATION) => [0.25, 0.65, 0.95, 0.95],
            Some(DiagnosticSeverity::HINT) => [0.55, 0.58, 0.65, 0.90],
            _ => [0.85, 0.35, 0.35, 0.95],
        };

        quad_instances.push(QuadInstance::solid(
            [x_start, y, width, 2.0 * scale_factor],
            color,
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn render_completion_popup(
    popup: &mut CompletionPopupState,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef,
    anchor_x: f32,
    anchor_y: f32,
    scale_factor: f32,
) {
    let item_h = 22.0 * scale_factor;
    let visible_items = popup.items.len().min(8);
    let card_w = 260.0 * scale_factor;
    let card_h = visible_items as f32 * item_h + 8.0 * scale_factor;

    let now = std::time::Instant::now();
    let reduced = crate::overlay_motion::reduced_motion();
    let draw_y = anchor_y + popup.motion.open_offset(now, reduced, -6.0 * scale_factor);
    popup.rect = [anchor_x, anchor_y, card_w, card_h];

    super::panel::push_rounded_surface(
        quad_instances,
        [anchor_x, draw_y, card_w, card_h],
        8.0 * scale_factor,
        color(active_theme().chrome.panel),
        color(active_theme().chrome.border),
        5.0 * scale_factor,
        scale_factor,
    );

    let font_size = 11.0 * scale_factor;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };

    let start_idx = popup.selected_index.saturating_sub(visible_items / 2);
    let end_idx = (start_idx + visible_items).min(popup.items.len());
    let rows_top = draw_y + 4.0 * scale_factor;
    let visual = popup.motion.row(popup.selected_index, now, reduced);
    super::panel::push_moving_row(
        quad_instances,
        anchor_x,
        rows_top,
        card_w,
        item_h,
        visual,
        start_idx,
        end_idx - start_idx,
        color(active_theme().chrome.panel_row),
        color(active_theme().accent),
        scale_factor,
    );

    for (display_idx, idx) in (start_idx..end_idx).enumerate() {
        let item_y = rows_top + display_idx as f32 * item_h;
        let is_selected = idx == popup.selected_index;

        let item = &popup.items[idx];
        let icon_size = 12.0 * scale_factor;
        let hole = if is_selected {
            color(active_theme().chrome.panel_row)
        } else {
            color(active_theme().chrome.panel)
        };
        super::icons::push_icon(
            completion_icon(item.kind),
            quad_instances,
            anchor_x + 8.0 * scale_factor,
            item_y + (item_h - icon_size) * 0.5,
            icon_size,
            color(active_theme().accent),
            hole,
        );
        let label_text = editor_view
            .text_layout_mut()
            .layout_line(&style, &item.label, &[]);
        let label_x = anchor_x + 8.0 * scale_factor + icon_size + 6.0 * scale_factor;
        let label_y = item_y + 3.0 * scale_factor;

        let text_color = if is_selected {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            color(active_theme().foreground)
        };

        for cluster in label_text.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                font_size,
                &cluster,
                label_x,
                label_y + label_text.baseline(),
                text_color,
                glyph_instances,
            );
        }
    }
    crate::overlay_motion::note_if_animating(&popup.motion, now);
}

fn completion_icon(kind: Option<CompletionItemKind>) -> super::icons::Icon {
    match kind {
        Some(CompletionItemKind::FUNCTION)
        | Some(CompletionItemKind::METHOD)
        | Some(CompletionItemKind::CONSTRUCTOR) => super::icons::Icon::Symbol,
        Some(CompletionItemKind::STRUCT)
        | Some(CompletionItemKind::CLASS)
        | Some(CompletionItemKind::INTERFACE)
        | Some(CompletionItemKind::ENUM)
        | Some(CompletionItemKind::MODULE) => super::icons::Icon::View,
        Some(CompletionItemKind::FILE) | Some(CompletionItemKind::FOLDER) => {
            super::icons::Icon::File
        }
        Some(CompletionItemKind::SNIPPET) => super::icons::Icon::Snippet,
        Some(CompletionItemKind::VARIABLE)
        | Some(CompletionItemKind::FIELD)
        | Some(CompletionItemKind::PROPERTY)
        | Some(CompletionItemKind::CONSTANT) => super::icons::Icon::Edit,
        _ => super::icons::Icon::Command,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_hover_popup(
    hover: &mut HoverPopupState,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef,
    anchor_x: f32,
    anchor_y: f32,
    scale_factor: f32,
) {
    let font_size = 11.0 * scale_factor;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };

    let first_line = hover.text.lines().next().unwrap_or("Documentation");
    let line_layout = editor_view
        .text_layout_mut()
        .layout_line(&style, first_line, &[]);

    let box_w = (line_layout.width() + 24.0 * scale_factor).min(400.0 * scale_factor);
    let box_h = 28.0 * scale_factor;

    hover.rect = [anchor_x, anchor_y, box_w, box_h];

    quad_instances.push(QuadInstance::solid(
        [anchor_x - 1.0, anchor_y - 1.0, box_w + 2.0, box_h + 2.0],
        color(active_theme().chrome.border),
    ));
    quad_instances.push(QuadInstance::solid(
        [anchor_x, anchor_y, box_w, box_h],
        color(active_theme().chrome.panel),
    ));

    let text_x = anchor_x + 8.0 * scale_factor;
    let text_y = anchor_y + 4.0 * scale_factor;
    for cluster in line_layout.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            text_x,
            text_y + line_layout.baseline(),
            color(active_theme().foreground),
            glyph_instances,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_problems_panel(
    problems: &mut ProblemsPanelState,
    all_diagnostics: &[(Url, Vec<DiagnosticItem>)],
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef,
    panel_x: f32,
    panel_w: f32,
    panel_y: f32,
    panel_h: f32,
    scale_factor: f32,
) {
    problems.rect = [panel_x, panel_y, panel_w, panel_h];

    // Background panel
    quad_instances.push(QuadInstance::solid(
        [panel_x, panel_y, panel_w, panel_h],
        color(active_theme().chrome.panel),
    ));
    let theme = active_theme();
    let stroke_color = if problems.focused {
        color(theme.accent)
    } else {
        color(theme.chrome.border)
    };
    quad_instances.push(QuadInstance::solid(
        [panel_x, panel_y, panel_w, 1.0],
        stroke_color,
    ));

    let font_size = 11.0 * scale_factor;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };

    let title_line = editor_view
        .text_layout_mut()
        .layout_line(&style, "Problems", &[]);
    for cluster in title_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            panel_x + 10.0 * scale_factor,
            panel_y + 4.0 * scale_factor + title_line.baseline(),
            color(active_theme().foreground),
            glyph_instances,
        );
    }

    let mut row_idx = 0;
    let row_h = 20.0 * scale_factor;
    let list_top = panel_y + 24.0 * scale_factor;

    for (url, diags) in all_diagnostics {
        let file_name = url.path().split('/').next_back().unwrap_or("file");
        for d in diags {
            if row_idx >= 6 {
                break;
            }
            let row_y = list_top + row_idx as f32 * row_h;
            let label = format!(
                "{} [{}:{}]: {}",
                file_name,
                d.range.start.line + 1,
                d.range.start.character + 1,
                d.message
            );
            let row_line = editor_view
                .text_layout_mut()
                .layout_line(&style, &label, &[]);

            let text_color = match d.severity {
                Some(DiagnosticSeverity::ERROR) | None => [0.95, 0.35, 0.35, 1.0],
                Some(DiagnosticSeverity::WARNING) => [0.90, 0.70, 0.20, 1.0],
                _ => [0.70, 0.75, 0.85, 1.0],
            };

            for cluster in row_line.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    font_size,
                    &cluster,
                    panel_x + 14.0 * scale_factor,
                    row_y + 2.0 * scale_factor + row_line.baseline(),
                    text_color,
                    glyph_instances,
                );
            }
            row_idx += 1;
        }
    }
}
