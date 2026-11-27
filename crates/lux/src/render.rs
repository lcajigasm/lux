mod about;
mod branch_picker;
mod build;
mod build_output;
mod clip;
mod conflict;
mod edit_menu;
mod git;
mod goto;
mod html_view;
mod icons;
mod ime;
mod language_picker;
mod lsp;
pub(crate) mod markdown;
pub(crate) mod minimap;
mod modals;
pub(crate) mod overlay_list;
pub(crate) mod paint;
mod palette;
mod pane;
pub(crate) mod panel;
mod plugin;
mod project_picker;
mod project_search;
pub(crate) mod search;
pub(crate) mod settings;
mod sidebar;
mod snippet_picker;
pub(crate) mod split;
mod status_bar;
mod tab_bar;
mod tab_list;
mod term;
pub(crate) mod text;
mod theme_picker;
mod theme_sample;
mod window;

use std::time::Instant;

use crate::presentation::Outcome;
use lux_platform::PlatformContext;
use lux_render::QuadInstance;
use paint::{DrawList, color, set_active_theme, set_hidpi_grayscale};

use crate::app::{App, BLINK_INTERVAL};

impl App {
    /// Paints one frame: the editor or the Markdown preview, then chrome.
    pub(crate) fn redraw(&mut self, ctx: &dyn PlatformContext) {
        crate::overlay_motion::begin_frame(self.chrome.motion.reduced);
        crate::goto::preview::settle(self);
        crate::tab_strip::follow_active(self);
        if !self.presentation.ready(Instant::now()) {
            return;
        }
        let Some(win_size) = self.gpu_window.as_ref().map(|g| g.window.inner_size()) else {
            return;
        };
        // The snapshot and the window share this layout: panels that opened
        // since the last frame shrink the editor before anything is drawn.
        let extent = crate::snapshot::frame_extent(win_size.width, win_size.height);
        let layout = self.fit_viewport(extent.width, extent.height);
        let tab_bar_height = self.tab_bar_height();
        let status_bar_height = self.status_bar_height();
        let tab_width = self.tab_width();
        let (strip_left, strip_width) = crate::tab_strip::span(self);
        let is_current_dirty = self.document_dirty();
        let editor_top = tab_bar_height;

        let minimap_document = self.minimap_document();
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
        let font_size = editor_view.metrics().font_size;
        let line_height = editor_view.metrics().line_height;
        let stroke = *scale_factor;

        let elapsed = Instant::now().duration_since(*start);
        let frame = editor_view.build_frame(buffer, cursors, viewport, elapsed, Some(fold_state));
        let is_df = ui.distraction_free.is_some();
        let sidebar_width = layout.editor[0];
        let window_width = extent.width;
        let window_height = extent.height;
        let gutter_total_width = if is_df {
            0.0
        } else {
            text::gutter_total_width(frame.gutter_width, *scale_factor)
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
        let mut overlay_quads = Vec::new();
        let mut overlay_glyphs = Vec::new();
        let mut gutter_quads = Vec::new();
        let mut gutter_glyphs = Vec::new();
        let mut cursor_screen = None;

        if ui.split.is_active() {
            let params = split::SplitRenderParams {
                split: &ui.split,
                documents,
                area: [
                    sidebar_width,
                    editor_top,
                    viewport.width + ui.minimap.effective_width(*scale_factor),
                    viewport.height,
                ],
                minimap_w: if is_df {
                    0.0
                } else {
                    ui.minimap.effective_width(*scale_factor)
                },
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
                gutter_hover: self.mouse.gutter_hover,
            };
            let minimap = split::SplitMinimap {
                state: &mut ui.minimap,
                marks: minimap::Marks {
                    git: &ui.git.markers,
                    search: ui.search_bar.as_ref(),
                },
            };
            split::render_split_layout(
                &params,
                Some(minimap),
                editor_view,
                atlas_state,
                &mut quad_instances,
                &mut glyph_instances,
                &mut gutter_quads,
                &mut gutter_glyphs,
            );
        } else {
            let pane = pane::SinglePane {
                frame: &frame,
                buffer,
                syntax: syntax.as_ref(),
                viewport,
                bookmarks,
                documents,
                search: ui.search_bar.as_ref(),
                syntax_theme: crate::color_scheme::syntax_theme(
                    &ui.settings.schemes,
                    chrome.theme.name,
                ),
                theme: &chrome.theme,
                font,
                font_size,
                line_height,
                stroke,
                scale_factor: *scale_factor,
                editor_top,
                sidebar_width,
                text_left,
                gutter_total_width,
                window_width,
                distraction_free: is_df,
                gutter_hover: self.mouse.gutter_hover,
            };
            let mut out = pane::PaneOutput {
                quads: &mut quad_instances,
                glyphs: &mut glyph_instances,
                gutter_quads: &mut gutter_quads,
                gutter_glyphs: &mut gutter_glyphs,
            };
            cursor_screen = pane.paint(editor_view, atlas_state, &mut out);
        }

        if tab_bar_height > 0.0 {
            tab_bar::render_tab_bar(
                documents,
                is_current_dirty,
                strip_left,
                strip_width,
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

        ime::paint_composition(
            ime,
            cursor_screen,
            gpu_window.window,
            editor_view,
            atlas_state,
            font,
            font_size,
            line_height,
            stroke,
            color(chrome.theme.foreground),
            color(chrome.theme.accent),
            &mut quad_instances,
            &mut glyph_instances,
        );
        let language_id = documents.language();
        if let (Some(search_state), Some(bar)) = (&mut ui.search_bar, layout.search_bar) {
            search::render_search_bar(
                search_state,
                bar,
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
            sidebar::render_sidebar(
                sidebar_state,
                sidebar_width,
                tab_bar_height,
                window_height - tab_bar_height - status_bar_height,
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
                cursors.primary(),
                language_id,
                ui.macros.is_recording,
                ui.git.branch_name.as_deref(),
                &mut quad_instances,
                &mut glyph_instances,
            );
        } else {
            ui.status_bar.layout = crate::status_bar::StatusBarLayout::default();
        }

        if !is_df && !ui.split.is_active() {
            let place = minimap::Placement {
                rect: minimap::window_rect(
                    ui.minimap.effective_width(*scale_factor),
                    window_width,
                    tab_bar_height,
                    editor_top + viewport.height,
                ),
                group: 0,
                scale_factor: *scale_factor,
                line_height,
            };
            minimap::paint_focused(
                ui,
                minimap_document,
                buffer,
                syntax.as_ref(),
                (bookmarks, fold_state),
                viewport,
                place,
                &chrome.theme,
                &mut quad_instances,
            );
        }
        ui.minimap.end_frame();
        let tab_list = crate::tab_strip::list::view(documents, is_current_dirty);
        modals::render_all_modals(
            ui,
            tab_list.as_ref(),
            viewport,
            window_width,
            window_height,
            sidebar_width,
            tab_bar_height,
            status_bar_height,
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
            &mut overlay_quads,
            &mut overlay_glyphs,
            self.mouse.context_menu.as_ref(),
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
            &mut gutter_quads,
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
            window_height,
        );

        term::render_term(
            &mut ui.term,
            &ui.build,
            &mut quad_instances,
            &mut glyph_instances,
            editor_view,
            &mut atlas_state.atlas,
            &mut atlas_state.dirty,
            &mut atlas_state.scale_cx,
            font,
            layout.dock,
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
                &mut overlay_quads,
                &mut overlay_glyphs,
                editor_view,
                &mut atlas_state.atlas,
                &mut atlas_state.dirty,
                &mut atlas_state.scale_cx,
                font,
                window_width,
                window_height,
                tab_bar_height,
                status_bar_height,
                *scale_factor,
                &chrome.theme,
                chrome.system_scheme,
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
            DrawList {
                quads: &quad_instances,
                glyphs: &glyph_instances,
            },
            DrawList {
                quads: &gutter_quads,
                glyphs: &gutter_glyphs,
            },
            DrawList {
                quads: &overlay_quads,
                glyphs: &overlay_glyphs,
            },
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
        crate::overlay_motion::schedule_wakeup(scheduler, ctx);
    }
}
