//! Frame drawing: window/GPU setup and the actual per-frame draw calls.
//! Kept apart from `crate::app`'s event handling only to stay under the
//! 600-line file limit — this is still `impl App`, just in its own file.

mod about;
mod build;
mod conflict;
mod git;
mod goto;
mod icons;
mod language_picker;
mod lsp;
mod minimap;
mod modals;
pub(crate) mod paint;
mod palette;
pub(crate) mod panel;
mod plugin;
mod project_picker;
mod project_search;
mod search;
mod settings;
mod sidebar;
mod snippet_picker;
pub(crate) mod split;
mod status_bar;
mod tab_bar;
mod term;
mod text;
mod theme_picker;
mod window;

use std::time::Instant;

use crate::presentation::Outcome;
use lux_platform::PlatformContext;
use lux_render::QuadInstance;
use paint::{color, push_glyph, set_active_theme, set_hidpi_grayscale};

use crate::app::{App, BLINK_INTERVAL, FONT_SIZE, LINE_HEIGHT, TEXT_LEFT_PADDING};

impl App {
    /// Paints one real frame: shapes only the visible lines (virtualized,
    /// same as `EditorView`'s own tests verify), rasterizes every visible
    /// glyph through the real atlas, and draws the background, selection,
    /// cursor, column rulers, indent guides, and — when IME composition is
    /// in progress — the real inline preedit text at the actual cursor
    /// position, all in one real `wgpu` submit + present.
    pub(crate) fn redraw(&mut self, ctx: &dyn PlatformContext) {
        crate::overlay_motion::begin_frame(self.chrome.motion.reduced);
        if !self.presentation.ready(Instant::now()) {
            return;
        }
        let tab_bar_height = self.tab_bar_height();
        let status_bar_height = self.status_bar_height();
        let tab_width = self.tab_width();
        let is_current_dirty = self.document_dirty();
        let editor_top = tab_bar_height;

        let App {
            gpu_window,
            editor_view,
            font,
            buffer,
            cursors,
            viewport,
            scheduler,
            ime,
            start,
            atlas_state,
            chrome,
            scale_factor,
            presentation,
            syntax,
            documents,
            ui,
            fold_state,
            bookmarks,
            ..
        } = self;
        let Some(gpu_window) = gpu_window.as_mut() else {
            return;
        };
        let font = *font;
        set_hidpi_grayscale(*scale_factor);
        set_active_theme(chrome.theme);
        let font_size = FONT_SIZE * *scale_factor;
        let line_height = LINE_HEIGHT * *scale_factor;
        let stroke = *scale_factor;

        let elapsed = Instant::now().duration_since(*start);
        let frame = editor_view.build_frame(buffer, cursors, viewport, elapsed, Some(fold_state));
        let is_df = ui.distraction_free.is_some();
        let sidebar_width = if !is_df
            && let Some(sidebar) = &ui.sidebar
            && sidebar.visible
            && sidebar.tree.is_some()
        {
            sidebar.width * *scale_factor
        } else {
            0.0
        };
        let win_size = gpu_window.window.inner_size();
        let extent = crate::snapshot::frame_extent(win_size.width, win_size.height);
        let window_width = extent.width;
        let window_height = extent.height;
        let gutter_total_width = if is_df {
            0.0
        } else {
            frame.gutter_width + 16.0 * *scale_factor
        };
        let text_left = if is_df {
            let col_chars = ui
                .distraction_free
                .as_ref()
                .map_or(80, |df| df.column_width_chars);
            let char_width = editor_view.reference_width("0");
            let content_width = char_width * col_chars as f32;
            ((window_width - content_width) / 2.0).max(24.0 * *scale_factor)
        } else {
            sidebar_width + gutter_total_width + 6.0 * *scale_factor
        };
        let foreground = color(chrome.theme.foreground);

        let mut quad_instances = Vec::new();
        if tab_bar_height > 0.0 {
            quad_instances.push(QuadInstance::solid(
                [0.0, 0.0, window_width, tab_bar_height],
                color(chrome.theme.chrome.tab_bar),
            ));
            quad_instances.push(QuadInstance::solid(
                [0.0, tab_bar_height - stroke, window_width, stroke],
                color(chrome.theme.chrome.border),
            ));
        }
        let mut glyph_instances = Vec::new();
        let mut cursor_screen = None;

        if ui.split.is_active() {
            let params = split::SplitRenderParams {
                split: &ui.split,
                documents,
                primary_rect: [sidebar_width, editor_top, viewport.width, viewport.height],
                scale_factor: *scale_factor,
                stroke,
                font_size,
                line_height,
                font,
                active_syntax: syntax.as_ref(),
                active_buffer: buffer,
                active_cursors: cursors,
                active_fold_state: fold_state,
                active_bookmarks: bookmarks,
                elapsed,
                active_scroll: (viewport.scroll_x, viewport.scroll_y),
                theme: &chrome.theme,
                syntax_theme: crate::color_scheme::syntax_theme(
                    &ui.settings.schemes,
                    chrome.theme.name,
                ),
            };
            split::render_split_layout(
                &params,
                editor_view,
                atlas_state,
                &mut quad_instances,
                &mut glyph_instances,
            );
        } else {
            quad_instances.push(QuadInstance::solid(
                [sidebar_width, editor_top, viewport.width, viewport.height],
                color(chrome.theme.background),
            ));
            if gutter_total_width > 0.0 {
                quad_instances.push(QuadInstance::solid(
                    [
                        sidebar_width,
                        editor_top,
                        gutter_total_width,
                        viewport.height,
                    ],
                    color(chrome.theme.chrome.gutter),
                ));
                quad_instances.push(QuadInstance::solid(
                    [
                        sidebar_width + gutter_total_width - stroke,
                        editor_top,
                        stroke,
                        viewport.height,
                    ],
                    color(chrome.theme.chrome.border),
                ));
            }

            for cursor in &frame.cursors {
                if cursor.primary {
                    let cur_y =
                        editor_top + cursor.visual_row as f32 * line_height - viewport.scroll_y;
                    if cur_y >= editor_top && cur_y < editor_top + viewport.height {
                        let h_x = if is_df {
                            0.0
                        } else {
                            sidebar_width + gutter_total_width
                        };
                        let h_w = if is_df {
                            window_width
                        } else {
                            viewport.width - gutter_total_width
                        };
                        quad_instances.push(QuadInstance::solid(
                            [h_x, cur_y, h_w, line_height],
                            color(chrome.theme.chrome.line_highlight),
                        ));
                    }
                }
            }

            text::render_text_lines(
                &frame,
                buffer,
                syntax.as_ref(),
                Some(bookmarks),
                viewport,
                editor_top,
                line_height,
                sidebar_width,
                text_left,
                stroke,
                !is_df,
                &chrome.theme,
                atlas_state,
                font,
                font_size,
                &mut quad_instances,
                &mut glyph_instances,
            );

            if let Some(search_state) = &ui.search_bar {
                search::render_search_matches(
                    search_state,
                    buffer,
                    editor_view,
                    viewport,
                    editor_top,
                    line_height,
                    TEXT_LEFT_PADDING * *scale_factor,
                    &mut quad_instances,
                );
            }

            let line_pad_y = (line_height - font_size) * 0.5;
            let caret_h = font_size + 2.0 * stroke;
            for cursor_glyph in &frame.cursors {
                let x = text_left + cursor_glyph.x - viewport.scroll_x;
                let y = editor_top + cursor_glyph.visual_row as f32 * line_height
                    - viewport.scroll_y
                    + line_pad_y
                    - stroke;
                if cursor_glyph.primary {
                    cursor_screen = Some((x, y));
                }
                if cursor_glyph.visible {
                    quad_instances.push(QuadInstance::solid(
                        [x, y, 2.0 * stroke, caret_h],
                        color(chrome.theme.accent),
                    ));
                }
            }
        }

        if tab_bar_height > 0.0 {
            tab_bar::render_tab_bar(
                documents,
                is_current_dirty,
                sidebar_width,
                tab_width,
                tab_bar_height,
                *scale_factor,
                stroke,
                font_size,
                font,
                editor_view,
                &mut atlas_state.atlas,
                &mut atlas_state.dirty,
                &mut atlas_state.scale_cx,
                &chrome.theme,
                &mut quad_instances,
                &mut glyph_instances,
            );
        }

        if ime.is_composing()
            && let Some((x, y)) = cursor_screen
        {
            let family = editor_view.family().to_string();
            let style = lux_render::LineStyle {
                family: &family,
                font_size,
                ligatures: false,
            };
            let preedit_text = ime.preedit_text().to_string();
            let layout = editor_view
                .text_layout_mut()
                .layout_line(&style, &preedit_text, &[]);
            for cluster in layout.clusters() {
                push_glyph(
                    &mut atlas_state.atlas,
                    &mut atlas_state.dirty,
                    &mut atlas_state.scale_cx,
                    font,
                    font_size,
                    &cluster,
                    x,
                    y + layout.baseline(),
                    foreground,
                    &mut glyph_instances,
                );
            }
            // A thin underline distinguishes in-progress composition from
            // already-committed text — the same visual convention every
            // desktop IME candidate UI already uses.
            quad_instances.push(QuadInstance::solid(
                [
                    x,
                    y + line_height - 2.0 * stroke,
                    layout.width().max(stroke),
                    2.0 * stroke,
                ],
                color(chrome.theme.accent),
            ));
            gpu_window.window.set_ime_cursor_area(
                dpi::PhysicalPosition::new(x as i32, y as i32),
                dpi::PhysicalSize::new(layout.width().max(stroke) as u32, line_height as u32),
            );
        } else if let Some((x, y)) = cursor_screen {
            gpu_window.window.set_ime_cursor_area(
                dpi::PhysicalPosition::new(x as i32, y as i32),
                dpi::PhysicalSize::new((2.0 * stroke) as u32, line_height as u32),
            );
        }

        let language_id = documents.language();

        if let Some(search_state) = &mut ui.search_bar {
            let search_bar_top = editor_top + viewport.height;
            search::render_search_bar(
                search_state,
                viewport.width,
                search_bar_top,
                *scale_factor,
                stroke,
                font_size,
                font,
                editor_view,
                &mut atlas_state.atlas,
                &mut atlas_state.dirty,
                &mut atlas_state.scale_cx,
                &chrome.theme,
                &mut quad_instances,
                &mut glyph_instances,
            );
        }

        if let Some(sidebar_state) = &mut ui.sidebar {
            let search_bar_h = ui
                .search_bar
                .as_ref()
                .map_or(0.0, |s| s.height(*scale_factor));
            sidebar::render_sidebar(
                sidebar_state,
                sidebar_width,
                tab_bar_height,
                viewport.height + search_bar_h,
                *scale_factor,
                stroke,
                font_size,
                font,
                editor_view,
                &mut atlas_state.atlas,
                &mut atlas_state.dirty,
                &mut atlas_state.scale_cx,
                &chrome.theme,
                &mut quad_instances,
                &mut glyph_instances,
            );
        }

        if status_bar_height > 0.0 {
            ui.status_bar.layout = status_bar::render_status_bar(
                window_width,
                window_height,
                *scale_factor,
                stroke,
                font_size,
                font,
                editor_view,
                &mut atlas_state.atlas,
                &mut atlas_state.dirty,
                &mut atlas_state.scale_cx,
                &chrome.theme,
                buffer,
                cursors.primary().head,
                language_id,
                ui.macros.is_recording,
                &mut quad_instances,
                &mut glyph_instances,
            );
        } else {
            ui.status_bar.layout = crate::status_bar::StatusBarLayout::default();
        }

        if !is_df {
            let search_info = ui
                .search_bar
                .as_ref()
                .map(|s| (s.search_bar.matches(), s.search_bar.current_index()));
            minimap::render_minimap(
                &ui.minimap,
                buffer,
                viewport,
                window_width,
                window_height,
                tab_bar_height,
                status_bar_height,
                *scale_factor,
                stroke,
                line_height,
                &chrome.theme,
                Some(fold_state),
                search_info,
                &mut quad_instances,
            );
        }
        modals::render_all_modals(
            ui,
            viewport,
            window_width,
            sidebar_width,
            tab_bar_height,
            *scale_factor,
            stroke,
            font_size,
            font,
            cursors,
            &chrome.theme,
            editor_view,
            &mut atlas_state.atlas,
            &mut atlas_state.dirty,
            &mut atlas_state.scale_cx,
            &mut quad_instances,
            &mut glyph_instances,
        );

        git::render_git(
            &mut ui.git,
            &mut quad_instances,
            &mut glyph_instances,
            editor_view,
            &mut atlas_state.atlas,
            &mut atlas_state.dirty,
            &mut atlas_state.scale_cx,
            font,
            sidebar_width,
            frame.gutter_width,
            viewport.width,
            viewport.height,
            editor_top,
            line_height,
            viewport.scroll_y,
            *scale_factor,
        );

        let (cx, cy) = cursor_screen.unwrap_or((text_left, editor_top));
        lsp::render_lsp(
            &mut ui.lsp,
            &mut quad_instances,
            &mut glyph_instances,
            editor_view,
            &mut atlas_state.atlas,
            &mut atlas_state.dirty,
            &mut atlas_state.scale_cx,
            font,
            sidebar_width,
            text_left,
            viewport.width,
            viewport.height,
            editor_top,
            line_height,
            viewport.scroll_x,
            viewport.scroll_y,
            *scale_factor,
            cx,
            cy,
        );

        term::render_term(
            &mut ui.term,
            &mut quad_instances,
            &mut glyph_instances,
            editor_view,
            &mut atlas_state.atlas,
            &mut atlas_state.dirty,
            &mut atlas_state.scale_cx,
            font,
            window_width,
            gpu_window.window.inner_size().height as f32,
            status_bar_height,
            *scale_factor,
            stroke,
        );

        plugin::render_plugin_modal(
            &ui.plugin,
            &mut quad_instances,
            &mut glyph_instances,
            editor_view,
            &mut atlas_state.atlas,
            &mut atlas_state.dirty,
            &mut atlas_state.scale_cx,
            font,
            window_width,
            gpu_window.window.inner_size().height as f32,
            *scale_factor,
            stroke,
        );

        if let Some(panel) = &ui.settings.panel {
            settings::render_settings_panel(
                panel,
                &mut quad_instances,
                &mut glyph_instances,
                editor_view,
                &mut atlas_state.atlas,
                &mut atlas_state.dirty,
                &mut atlas_state.scale_cx,
                font,
                window_width,
                gpu_window.window.inner_size().height as f32,
                tab_bar_height,
                *scale_factor,
                stroke,
                &chrome.theme,
            );
        }

        if atlas_state.dirty {
            gpu_window
                .text
                .upload_atlas(&gpu_window.gpu.queue, &atlas_state.atlas);
            atlas_state.dirty = false;
        }

        let outcome = crate::snapshot::present_or_capture(
            gpu_window,
            &quad_instances,
            &glyph_instances,
            &atlas_state.atlas,
            color(chrome.theme.background),
            extent.capture,
        );
        if let Some(at) = presentation.finish(outcome, scheduler, Instant::now()) {
            ctx.request_wakeup(at);
        }
        if !matches!(outcome, Outcome::Presented) {
            return;
        }

        if chrome.motion.cursor_blinks() {
            let phase = elapsed.as_millis() / BLINK_INTERVAL.as_millis().max(1);
            let next_toggle = *start + BLINK_INTERVAL * (phase as u32 + 1);
            scheduler.request_repaint_at(next_toggle);
            ctx.request_wakeup(next_toggle);
        }
        if crate::overlay_motion::take_animating() {
            let at = Instant::now() + std::time::Duration::from_millis(16);
            scheduler.request_repaint_at(at);
            ctx.request_wakeup(at);
        }
    }
}
