//! Rendering logic for the In-Buffer Find & Replace Bar (G10).

use swash::FontRef;
use swash::scale::ScaleContext;

use crate::render::paint::{color, push_glyph};
use crate::search::{SearchBarLayout, SearchBarState, SearchField};
use lux_core::buffer::Buffer;
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_search_matches(
    search: &SearchBarState,
    buffer: &Buffer,
    editor_view: &mut EditorView,
    viewport: &lux_ui::editor_view::Viewport,
    editor_top: f32,
    line_height: f32,
    left_padding: f32,
    quad_instances: &mut Vec<QuadInstance>,
) {
    let current_index = search.search_bar.current_index();
    for (i, m) in search.search_bar.matches().iter().enumerate() {
        let line = m.head.line;
        let y = editor_top + line as f32 * line_height - viewport.scroll_y;
        if y + line_height < editor_top || y > editor_top + viewport.height {
            continue;
        }

        let x_start = editor_view.x_for_position(buffer, m.anchor, left_padding);
        let x_end = editor_view.x_for_position(buffer, m.head, left_padding);
        let min_x = x_start.min(x_end) - viewport.scroll_x;
        let max_x = x_start.max(x_end) - viewport.scroll_x;
        let width = (max_x - min_x).max(2.0);

        let is_current = current_index == Some(i);
        let bg_color = if is_current {
            [0.72, 0.52, 0.12, 0.55]
        } else {
            [0.45, 0.45, 0.15, 0.35]
        };

        quad_instances.push(QuadInstance {
            rect: [min_x, y, width, line_height],
            color: bg_color,
        });

        if is_current {
            // Outline border for active match
            quad_instances.push(QuadInstance {
                rect: [min_x, y + line_height - 2.0, width, 2.0],
                color: [0.95, 0.80, 0.20, 1.0],
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_search_bar(
    search: &mut SearchBarState,
    viewport_width: f32,
    search_bar_top: f32,
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
    let bar_height = search.height(scale_factor);
    let bar_width = viewport_width;
    let bar_top = search_bar_top;

    search.layout = SearchBarLayout {
        bar_rect: [0.0, bar_top, bar_width, bar_height],
        ..Default::default()
    };

    let bar_font_size = font_size * 0.78;
    let family = editor_view.family().to_string();
    let style = lux_render::LineStyle {
        family: &family,
        font_size: bar_font_size,
        ligatures: false,
    };

    let foreground = color(theme.foreground);
    let muted = color(theme.muted);
    let accent = color(theme.accent);

    // 1. Dock Bar Background
    quad_instances.push(QuadInstance {
        rect: [0.0, bar_top, bar_width, bar_height],
        color: color(theme.chrome.panel),
    });

    // 2. Top Border
    quad_instances.push(QuadInstance {
        rect: [0.0, bar_top, bar_width, stroke],
        color: color(theme.chrome.border),
    });

    let input_h = 24.0 * scale_factor;
    let input_w = (viewport_width * 0.35).clamp(180.0 * scale_factor, 320.0 * scale_factor);
    let mut cur_x = 12.0 * scale_factor;
    let row1_y = bar_top + 5.0 * scale_factor;

    // --- ROW 1: FIND ---
    search.layout.find_input_rect = [cur_x, row1_y, input_w, input_h];
    let find_focused = search.focused_field == SearchField::Find;

    // Find input box background & border
    quad_instances.push(QuadInstance {
        rect: [cur_x, row1_y, input_w, input_h],
        color: color(theme.chrome.panel_input),
    });
    let find_border_color = if find_focused {
        accent
    } else {
        color(theme.chrome.border)
    };
    quad_instances.push(QuadInstance {
        rect: [cur_x, row1_y, input_w, stroke],
        color: find_border_color,
    });
    quad_instances.push(QuadInstance {
        rect: [cur_x, row1_y + input_h - stroke, input_w, stroke],
        color: find_border_color,
    });
    quad_instances.push(QuadInstance {
        rect: [cur_x, row1_y, stroke, input_h],
        color: find_border_color,
    });
    quad_instances.push(QuadInstance {
        rect: [cur_x + input_w - stroke, row1_y, stroke, input_h],
        color: find_border_color,
    });

    // Find text & caret
    let text_x = cur_x + 6.0 * scale_factor;
    let baseline = (input_h - bar_font_size) * 0.5;
    let (disp_text, txt_col) = if search.find_query.is_empty() {
        ("Find", muted)
    } else {
        (search.find_query.as_str(), foreground)
    };
    let line = editor_view
        .text_layout_mut()
        .layout_line(&style, disp_text, &[]);
    for c in line.clusters() {
        push_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            bar_font_size,
            &c,
            text_x,
            row1_y + baseline + line.baseline(),
            txt_col,
            glyph_instances,
        );
    }

    if find_focused {
        let caret_x = if search.find_query.is_empty() {
            text_x
        } else {
            text_x + line.width() + 2.0 * stroke
        };
        quad_instances.push(QuadInstance {
            rect: [
                caret_x,
                row1_y + 3.0 * scale_factor,
                2.0 * stroke,
                input_h - 6.0 * scale_factor,
            ],
            color: accent,
        });
    }

    cur_x += input_w + 10.0 * scale_factor;

    // Match Counter Badge
    let total_matches = search.search_bar.matches().len();
    let counter_text = if search.find_query.is_empty() {
        String::new()
    } else if total_matches == 0 {
        "No matches".to_string()
    } else {
        let cur = search.search_bar.current_index().map_or(1, |i| i + 1);
        let in_sel = if search.in_selection { " in sel" } else { "" };
        format!("{cur} of {total_matches} matches{in_sel}")
    };
    if !counter_text.is_empty() {
        let count_line = editor_view
            .text_layout_mut()
            .layout_line(&style, &counter_text, &[]);
        for c in count_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                bar_font_size,
                &c,
                cur_x,
                row1_y + baseline + count_line.baseline(),
                muted,
                glyph_instances,
            );
        }
        cur_x += count_line.width() + 12.0 * scale_factor;
    }

    // Toggle Buttons (Aa, Word, Regex, In Selection)
    let btn_w = 26.0 * scale_factor;
    let draw_toggle = |quads: &mut Vec<QuadInstance>,
                       glyphs: &mut Vec<GlyphInstance>,
                       ed_view: &mut EditorView,
                       atl: &mut GlyphAtlas,
                       atl_dirty: &mut bool,
                       sc_cx: &mut ScaleContext,
                       label: &str,
                       active: bool,
                       rect: [f32; 4]| {
        let [rx, ry, rw, rh] = rect;
        let bg = if active {
            color(theme.chrome.panel_row)
        } else {
            color(theme.chrome.panel_input)
        };
        quads.push(QuadInstance { rect, color: bg });
        let txt_color = if active { accent } else { muted };
        let l = ed_view.text_layout_mut().layout_line(&style, label, &[]);
        let tx = rx + (rw - l.width()) * 0.5;
        let ty = ry + (rh - bar_font_size) * 0.5 + l.baseline();
        for c in l.clusters() {
            push_glyph(
                atl,
                atl_dirty,
                sc_cx,
                font,
                bar_font_size,
                &c,
                tx,
                ty,
                txt_color,
                glyphs,
            );
        }
    };

    search.layout.case_btn_rect = [cur_x, row1_y, btn_w, input_h];
    draw_toggle(
        quad_instances,
        glyph_instances,
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        "Aa",
        search.case_sensitive,
        search.layout.case_btn_rect,
    );
    cur_x += btn_w + 4.0 * scale_factor;

    search.layout.word_btn_rect = [cur_x, row1_y, btn_w, input_h];
    draw_toggle(
        quad_instances,
        glyph_instances,
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        "\"w\"",
        search.whole_word,
        search.layout.word_btn_rect,
    );
    cur_x += btn_w + 4.0 * scale_factor;

    search.layout.regex_btn_rect = [cur_x, row1_y, btn_w, input_h];
    draw_toggle(
        quad_instances,
        glyph_instances,
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        ".*",
        search.regex,
        search.layout.regex_btn_rect,
    );
    cur_x += btn_w + 4.0 * scale_factor;

    let sel_btn_w = 46.0 * scale_factor;
    search.layout.selection_btn_rect = [cur_x, row1_y, sel_btn_w, input_h];
    draw_toggle(
        quad_instances,
        glyph_instances,
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        "In Sel",
        search.in_selection,
        search.layout.selection_btn_rect,
    );
    cur_x += sel_btn_w + 10.0 * scale_factor;

    // Prev & Next Buttons
    search.layout.prev_btn_rect = [cur_x, row1_y, btn_w, input_h];
    draw_toggle(
        quad_instances,
        glyph_instances,
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        "<",
        false,
        search.layout.prev_btn_rect,
    );
    cur_x += btn_w + 4.0 * scale_factor;

    search.layout.next_btn_rect = [cur_x, row1_y, btn_w, input_h];
    draw_toggle(
        quad_instances,
        glyph_instances,
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        ">",
        false,
        search.layout.next_btn_rect,
    );

    // Close button (X) on far right
    let close_x = bar_width - btn_w - 12.0 * scale_factor;
    search.layout.close_btn_rect = [close_x, row1_y, btn_w, input_h];
    draw_toggle(
        quad_instances,
        glyph_instances,
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        "X",
        false,
        search.layout.close_btn_rect,
    );

    // --- ROW 2: REPLACE (if replace_mode) ---
    if search.replace_mode {
        let row2_y = bar_top + 34.0 * scale_factor;
        let mut rep_x = 12.0 * scale_factor;
        search.layout.replace_input_rect = [rep_x, row2_y, input_w, input_h];
        let rep_focused = search.focused_field == SearchField::Replace;

        quad_instances.push(QuadInstance {
            rect: [rep_x, row2_y, input_w, input_h],
            color: color(theme.chrome.panel_input),
        });
        let rep_border_color = if rep_focused {
            accent
        } else {
            color(theme.chrome.border)
        };
        quad_instances.push(QuadInstance {
            rect: [rep_x, row2_y, input_w, stroke],
            color: rep_border_color,
        });
        quad_instances.push(QuadInstance {
            rect: [rep_x, row2_y + input_h - stroke, input_w, stroke],
            color: rep_border_color,
        });
        quad_instances.push(QuadInstance {
            rect: [rep_x, row2_y, stroke, input_h],
            color: rep_border_color,
        });
        quad_instances.push(QuadInstance {
            rect: [rep_x + input_w - stroke, row2_y, stroke, input_h],
            color: rep_border_color,
        });

        let rep_text_x = rep_x + 6.0 * scale_factor;
        let (rep_disp, rep_col) = if search.replace_text.is_empty() {
            ("Replace", muted)
        } else {
            (search.replace_text.as_str(), foreground)
        };
        let rep_line = editor_view
            .text_layout_mut()
            .layout_line(&style, rep_disp, &[]);
        for c in rep_line.clusters() {
            push_glyph(
                atlas,
                atlas_dirty,
                scale_cx,
                font,
                bar_font_size,
                &c,
                rep_text_x,
                row2_y + baseline + rep_line.baseline(),
                rep_col,
                glyph_instances,
            );
        }

        if rep_focused {
            let caret_x = if search.replace_text.is_empty() {
                rep_text_x
            } else {
                rep_text_x + rep_line.width() + 2.0 * stroke
            };
            quad_instances.push(QuadInstance {
                rect: [
                    caret_x,
                    row2_y + 3.0 * scale_factor,
                    2.0 * stroke,
                    input_h - 6.0 * scale_factor,
                ],
                color: accent,
            });
        }

        rep_x += input_w + 10.0 * scale_factor;
        let find_btn_w = 46.0 * scale_factor;

        search.layout.find_btn_rect = [rep_x, row2_y, find_btn_w, input_h];
        draw_toggle(
            quad_instances,
            glyph_instances,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            "Find",
            false,
            search.layout.find_btn_rect,
        );
        rep_x += find_btn_w + 6.0 * scale_factor;

        let find_all_btn_w = 64.0 * scale_factor;
        search.layout.find_all_btn_rect = [rep_x, row2_y, find_all_btn_w, input_h];
        draw_toggle(
            quad_instances,
            glyph_instances,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            "Find All",
            false,
            search.layout.find_all_btn_rect,
        );
        rep_x += find_all_btn_w + 6.0 * scale_factor;

        let action_btn_w = 64.0 * scale_factor;
        search.layout.replace_btn_rect = [rep_x, row2_y, action_btn_w, input_h];
        draw_toggle(
            quad_instances,
            glyph_instances,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            "Replace",
            false,
            search.layout.replace_btn_rect,
        );
        rep_x += action_btn_w + 6.0 * scale_factor;

        let all_btn_w = 78.0 * scale_factor;
        search.layout.replace_all_btn_rect = [rep_x, row2_y, all_btn_w, input_h];
        draw_toggle(
            quad_instances,
            glyph_instances,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            "Replace All",
            false,
            search.layout.replace_all_btn_rect,
        );
    }
}
