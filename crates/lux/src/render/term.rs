//! Rendering logic for the integrated bottom dock terminal panel.

use swash::FontRef;
use swash::scale::ScaleContext;

use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

use crate::render::paint::{active_theme, color, push_glyph};
use crate::term::TerminalState;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_term(
    term: &mut TerminalState,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    window_width: f32,
    window_height: f32,
    status_bar_height: f32,
    scale_factor: f32,
    stroke: f32,
) {
    if !term.visible {
        return;
    }

    let dock_h = term.height * scale_factor;
    let dock_y = (window_height - status_bar_height - dock_h).max(0.0);
    let header_h = 26.0 * scale_factor;
    let grid_y = dock_y + header_h;
    let grid_h = (dock_h - header_h).max(1.0);

    // 1. Panel background
    quad_instances.push(QuadInstance::solid(
        [0.0, grid_y, window_width, grid_h],
        color(active_theme().background),
    ));

    // 2. Header background & border
    quad_instances.push(QuadInstance::solid(
        [0.0, dock_y, window_width, header_h],
        color(active_theme().chrome.tab_bar),
    ));
    let top_border_color = if term.focused {
        color(active_theme().accent)
    } else {
        color(active_theme().chrome.border)
    };
    quad_instances.push(QuadInstance::solid(
        [0.0, dock_y, window_width, stroke],
        top_border_color,
    ));
    quad_instances.push(QuadInstance::solid(
        [0.0, grid_y - stroke, window_width, stroke],
        color(active_theme().chrome.border),
    ));

    let font_size = 12.0 * scale_factor;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };

    // 3. Header tabs
    let mut tab_x = 8.0 * scale_factor;
    let tab_w = 90.0 * scale_factor;
    let active_color = color(active_theme().chrome.panel_row);
    let inactive_color = color(active_theme().chrome.tab_inactive);
    let text_fg = [0.85, 0.85, 0.90, 1.0];
    let muted_fg = [0.55, 0.55, 0.60, 1.0];

    for (i, name) in term.session_names.iter().enumerate() {
        let is_active = !term.show_tasks && term.active_tab == i;
        let tab_bg = if is_active {
            active_color
        } else {
            inactive_color
        };

        quad_instances.push(QuadInstance::solid(
            [tab_x, dock_y + stroke, tab_w, header_h - stroke],
            tab_bg,
        ));

        if is_active {
            quad_instances.push(QuadInstance::solid(
                [tab_x, dock_y, tab_w, 2.0 * stroke],
                color(active_theme().accent),
            ));
        }

        // Title text
        let title_line = editor_view.text_layout_mut().layout_line(&style, name, &[]);
        let title_base = title_line.baseline();
        for cluster in title_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                font_size,
                &cluster,
                tab_x + 8.0 * scale_factor,
                dock_y + (header_h - font_size) * 0.5 + title_base,
                if is_active { text_fg } else { muted_fg },
                glyph_instances,
            );
        }

        // Close '×' button
        let x_line = editor_view.text_layout_mut().layout_line(&style, "×", &[]);
        let x_base = x_line.baseline();
        for cluster in x_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                font_size,
                &cluster,
                tab_x + tab_w - 14.0 * scale_factor,
                dock_y + (header_h - font_size) * 0.5 + x_base,
                muted_fg,
                glyph_instances,
            );
        }

        tab_x += tab_w + 4.0 * scale_factor;
    }

    // '+' New tab button
    quad_instances.push(QuadInstance::solid(
        [
            tab_x,
            dock_y + 2.0 * stroke,
            24.0 * scale_factor,
            header_h - 4.0 * stroke,
        ],
        color(active_theme().chrome.panel_input),
    ));
    let plus_line = editor_view.text_layout_mut().layout_line(&style, "+", &[]);
    let plus_base = plus_line.baseline();
    for cluster in plus_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            tab_x + 8.0 * scale_factor,
            dock_y + (header_h - font_size) * 0.5 + plus_base,
            text_fg,
            glyph_instances,
        );
    }
    tab_x += 28.0 * scale_factor;

    // 'Tasks' button
    let tasks_bg = if term.show_tasks {
        active_color
    } else {
        inactive_color
    };
    quad_instances.push(QuadInstance::solid(
        [
            tab_x,
            dock_y + stroke,
            55.0 * scale_factor,
            header_h - stroke,
        ],
        tasks_bg,
    ));
    let tasks_line = editor_view
        .text_layout_mut()
        .layout_line(&style, "Tasks", &[]);
    let tasks_base = tasks_line.baseline();
    for cluster in tasks_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            tab_x + 8.0 * scale_factor,
            dock_y + (header_h - font_size) * 0.5 + tasks_base,
            if term.show_tasks { text_fg } else { muted_fg },
            glyph_instances,
        );
    }

    // Close panel button on far right
    let close_x = window_width - 24.0 * scale_factor;
    let close_line = editor_view.text_layout_mut().layout_line(&style, "✕", &[]);
    let close_base = close_line.baseline();
    for cluster in close_line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            close_x,
            dock_y + (header_h - font_size) * 0.5 + close_base,
            muted_fg,
            glyph_instances,
        );
    }

    // 4. Content: Tasks or PTY Grid
    if term.show_tasks {
        render_tasks_view(
            term,
            quad_instances,
            glyph_instances,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            grid_y,
            scale_factor,
            stroke,
        );
    } else {
        render_pty_grid(
            term,
            quad_instances,
            glyph_instances,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            window_width,
            grid_y,
            grid_h,
            scale_factor,
            stroke,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_tasks_view(
    term: &TerminalState,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    grid_y: f32,
    scale_factor: f32,
    _stroke: f32,
) {
    let font_size = 12.0 * scale_factor;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };

    let title = "Background Project Tasks:";
    let line = editor_view
        .text_layout_mut()
        .layout_line(&style, title, &[]);
    let base = line.baseline();
    for cluster in line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &cluster,
            16.0 * scale_factor,
            grid_y + 16.0 * scale_factor + base,
            [0.90, 0.90, 0.95, 1.0],
            glyph_instances,
        );
    }

    if term.task_ids.is_empty() {
        let msg = "No project tasks running. Start a task with 'Terminal: Run Project Task'.";
        let line = editor_view.text_layout_mut().layout_line(&style, msg, &[]);
        let base = line.baseline();
        for cluster in line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                font_size,
                &cluster,
                16.0 * scale_factor,
                grid_y + 40.0 * scale_factor + base,
                [0.55, 0.55, 0.60, 1.0],
                glyph_instances,
            );
        }
    } else {
        let mut row_y = grid_y + 40.0 * scale_factor;
        for id in &term.task_ids {
            let name = term.tasks.name(*id).unwrap_or("Task");
            let label = format!("{name}  [RUNNING]");
            let line = editor_view
                .text_layout_mut()
                .layout_line(&style, &label, &[]);
            let base = line.baseline();
            for cluster in line.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    font_size,
                    &cluster,
                    16.0 * scale_factor,
                    row_y + base,
                    [0.30, 0.85, 0.45, 1.0],
                    glyph_instances,
                );
            }

            // Cancel button
            quad_instances.push(QuadInstance::solid(
                [
                    180.0 * scale_factor,
                    row_y - 2.0 * scale_factor,
                    60.0 * scale_factor,
                    18.0 * scale_factor,
                ],
                [0.75, 0.20, 0.20, 1.0],
            ));
            let cancel_line = editor_view
                .text_layout_mut()
                .layout_line(&style, "Cancel", &[]);
            let c_base = cancel_line.baseline();
            for cluster in cancel_line.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    font_size * 0.9,
                    &cluster,
                    190.0 * scale_factor,
                    row_y + c_base,
                    [1.0, 1.0, 1.0, 1.0],
                    glyph_instances,
                );
            }

            row_y += 24.0 * scale_factor;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_pty_grid(
    term: &mut TerminalState,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    window_width: f32,
    grid_y: f32,
    grid_h: f32,
    scale_factor: f32,
    stroke: f32,
) {
    let font_size = 12.0 * scale_factor;
    let char_w = 7.2 * scale_factor;
    let row_h = 16.0 * scale_factor;

    let cols = ((window_width - 16.0 * scale_factor) / char_w)
        .floor()
        .max(10.0) as u16;
    let rows = ((grid_h - 8.0 * scale_factor) / row_h).floor().max(4.0) as u16;
    term.resize(rows, cols);

    let Some(pty) = term.active_pty() else {
        return;
    };

    let visible_cells = pty.visible_cells();
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };

    let start_x = 8.0 * scale_factor;
    let start_y = grid_y + 4.0 * scale_factor;

    // Render cells
    for (r, row_cells) in visible_cells.iter().enumerate() {
        if r >= rows as usize {
            break;
        }
        let y = start_y + r as f32 * row_h;

        // Build plain text for this row to shape efficiently
        let mut line_text = String::with_capacity(row_cells.len());
        for cell in row_cells {
            line_text.push(cell.c);
        }
        let trimmed_text = line_text.trim_end();
        if trimmed_text.is_empty() {
            continue;
        }

        let line = editor_view
            .text_layout_mut()
            .layout_line(&style, trimmed_text, &[]);
        let base = line.baseline();

        for cluster in line.clusters() {
            let col_idx = (cluster.x / char_w).floor() as usize;
            let (fg_color, bg_color) = if let Some(cell) = row_cells.get(col_idx) {
                let fg = cell
                    .fg
                    .map(|[r, g, b]| [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0])
                    .unwrap_or([0.88, 0.88, 0.92, 1.0]);
                let bg = cell
                    .bg
                    .map(|[r, g, b]| [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]);
                (fg, bg)
            } else {
                ([0.88, 0.88, 0.92, 1.0], None)
            };

            if let Some(bg) = bg_color {
                quad_instances.push(QuadInstance::solid(
                    [start_x + cluster.x, y, char_w, row_h],
                    bg,
                ));
            }

            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                font_size,
                &cluster,
                start_x,
                y + base,
                fg_color,
                glyph_instances,
            );
        }
    }

    // Render cursor
    let (cur_row, cur_col) = pty.cursor_position();
    if cur_row < rows as usize && cur_col < cols as usize {
        let cur_x = start_x + cur_col as f32 * char_w;
        let cur_y = start_y + cur_row as f32 * row_h;
        quad_instances.push(QuadInstance::solid(
            [cur_x, cur_y, char_w, row_h],
            color(active_theme().accent),
        ));
    }

    // Underline detected error/file links
    let links = pty.detect_links();
    let snap = pty.snapshot();
    let lines: Vec<&str> = snap.lines().collect();
    for (r, line) in lines.iter().enumerate() {
        if r >= rows as usize {
            break;
        }
        for link in &links {
            let fname = link.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !fname.is_empty()
                && let Some(col_start) = line.find(fname)
            {
                let link_x = start_x + col_start as f32 * char_w;
                let link_w = fname.len() as f32 * char_w;
                let link_y = start_y + (r + 1) as f32 * row_h - stroke;
                quad_instances.push(QuadInstance::solid(
                    [link_x, link_y, link_w, stroke * 1.5],
                    [0.95, 0.70, 0.25, 0.85],
                ));
            }
        }
    }
}
