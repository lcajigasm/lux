//! Rendering logic for the integrated bottom dock terminal panel.

use swash::FontRef;
use swash::scale::ScaleContext;

use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

use crate::render::clip::{clip_glyphs, clip_quads};
use crate::render::paint::{active_theme, color, push_glyph};
use crate::term::TerminalState;
use crate::term::dock;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_term(
    term: &mut TerminalState,
    build: &crate::build::BuildState,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    dock_rect: Option<[f32; 4]>,
    scale_factor: f32,
    stroke: f32,
) {
    let Some(rect) = dock_rect.filter(|_| term.visible) else {
        return;
    };
    // Everything below is drawn in the dock's own column and then cut to
    // it, so nothing spills into the sidebar or past the window.
    let (first_quad, first_glyph) = (quad_instances.len(), glyph_instances.len());
    let [dock_x, dock_y, window_width, dock_h] = rect;
    let header_h = dock::HEADER_HEIGHT * scale_factor;
    let grid_y = dock::grid_top(rect, scale_factor);
    let grid_h = (dock_h - header_h).max(1.0);

    // 1. Panel background
    quad_instances.push(QuadInstance::solid(
        [dock_x, grid_y, window_width, grid_h],
        color(active_theme().background),
    ));

    // 2. Header: the tab well, a hairline under it, and flat labels. The
    // active tab is foreground with a 2 pt accent underline; the others
    // are muted. There are no per-tab fills.
    let theme = active_theme();
    let well = color(theme.chrome.tab_bar);
    quad_instances.push(QuadInstance::solid(
        [dock_x, dock_y, window_width, header_h],
        well,
    ));
    let rule = color(theme.chrome.border);
    quad_instances.push(QuadInstance::solid(
        [dock_x, dock_y, window_width, stroke],
        rule,
    ));
    quad_instances.push(QuadInstance::solid(
        [dock_x, grid_y - stroke, window_width, stroke],
        rule,
    ));
    let slots = dock::header_slots(rect, term.session_names.len(), scale_factor);
    let active_ink = super::panel::label_ink(&theme, well);
    let muted_ink = super::panel::hint_ink(&theme, well);
    let size = super::panel::chrome_text::PANEL_TAB_PT * scale_factor;
    let mid = dock_y + header_h * 0.5;
    let label_x = |slot: [f32; 4]| slot[0] + 8.0 * scale_factor;
    let mut underline = None;
    {
        let mut pen = super::panel::chrome_text::ChromePen::new(
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            glyph_instances,
        );
        for (i, name) in term.session_names.iter().enumerate() {
            let slot = slots.tabs[i];
            let active = !term.show_tasks && term.active_tab == i;
            let ink = if active { active_ink } else { muted_ink };
            let width = pen.draw(size, name, label_x(slot), mid, ink);
            if active {
                underline = Some((label_x(slot), width));
            }
            pen.draw(
                size,
                "×",
                slot[0] + slot[2] - 14.0 * scale_factor,
                mid,
                muted_ink,
            );
        }
        let plus_w = pen.width(size, 0.0, "+");
        pen.draw(
            size,
            "+",
            slots.plus[0] + (slots.plus[2] - plus_w) * 0.5,
            mid,
            muted_ink,
        );
        let tasks_ink = if term.show_tasks {
            active_ink
        } else {
            muted_ink
        };
        let width = pen.draw(size, "Tasks", label_x(slots.tasks), mid, tasks_ink);
        if term.show_tasks {
            underline = Some((label_x(slots.tasks), width));
        }
        let close_w = pen.width(size, 0.0, "×");
        pen.draw(
            size,
            "×",
            slots.close[0] + (slots.close[2] - close_w) * 0.5,
            mid,
            muted_ink,
        );
    }
    if let Some((x, width)) = underline {
        quad_instances.push(QuadInstance::solid(
            [x, grid_y - 2.0 * stroke, width, 2.0 * stroke],
            color(theme.accent),
        ));
    }

    // 4. Content: the shell grid, the active task's output, or the
    // empty Tasks view.
    if term.show_tasks && term.active_task_pty().is_none() {
        render_tasks_view(
            quad_instances,
            glyph_instances,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            rect,
            scale_factor,
        );
    } else {
        let marks = if term.show_tasks && term.active_task == build.task_id {
            Some(super::build_output::LineMarks::new(build))
        } else {
            None
        };
        render_pty_grid(
            term,
            marks.as_ref(),
            quad_instances,
            glyph_instances,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            [dock_x, grid_y, window_width, grid_h],
            scale_factor,
            stroke,
        );
    }
    clip_quads(quad_instances, first_quad, rect);
    clip_glyphs(glyph_instances, first_glyph, rect);
}

/// The Tasks view before any task has run.
#[allow(clippy::too_many_arguments)]
fn render_tasks_view(
    _quad_instances: &mut [QuadInstance],
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    rect: [f32; 4],
    scale_factor: f32,
) {
    let font_size = 12.0 * scale_factor;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };
    // Empty state inside #164's dock rectangle, in the theme's hint ink.
    let grid_y = dock::grid_top(rect, scale_factor);
    let theme = &active_theme();
    let hint = super::panel::hint_ink(theme, color(theme.background));
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
            rect[0] + 16.0 * scale_factor,
            grid_y + 16.0 * scale_factor + base,
            hint,
            glyph_instances,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_pty_grid(
    term: &mut TerminalState,
    marks: Option<&super::build_output::LineMarks<'_>>,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    font: FontRef<'static>,
    grid: [f32; 4],
    scale_factor: f32,
    stroke: f32,
) {
    let [grid_x, grid_y, window_width, grid_h] = grid;
    let font_size = 12.0 * scale_factor;
    let char_w = 7.2 * scale_factor;
    let row_h = 16.0 * scale_factor;

    let cols = ((window_width - 16.0 * scale_factor) / char_w)
        .floor()
        .max(10.0) as u16;
    let rows = ((grid_h - 8.0 * scale_factor) / row_h).floor().max(4.0) as u16;
    term.resize(rows, cols);

    let pty = if marks.is_some() {
        term.active_task_pty()
    } else {
        term.active_pty()
    };
    let Some(pty) = pty else {
        return;
    };

    let visible_cells = pty.visible_cells();
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size,
        ligatures: false,
    };

    let start_x = grid_x + 8.0 * scale_factor;
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
        let marked_ink = marks.and_then(|marks| marks.ink(trimmed_text, &active_theme()));

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
            let fg_color = marked_ink.unwrap_or(fg_color);

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
