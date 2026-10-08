//! Frame drawing: window/GPU setup and the actual per-frame draw calls.
//! Kept apart from `crate::app`'s event handling only to stay under the
//! 600-line file limit — this is still `impl App`, just in its own file.

mod paint;

use std::time::Instant;

use crate::presentation::Outcome;
use dpi::{LogicalSize, PhysicalSize};
use lux_platform::{PlatformContext, PlatformWindow, ScrollDelta, WindowAttributes};
use lux_render::{GpuContext, QuadInstance, QuadPipeline, Surface, TextPipeline};
use lux_ui::Theme;
use paint::{color, paint, push_glyph, syntax_color};

use crate::app::{
    ATLAS_SIDE, App, BLINK_INTERVAL, FONT_SIZE, GpuWindow, LINE_HEIGHT, TEXT_LEFT_PADDING,
};

impl App {
    pub(crate) fn create_gpu_window(&mut self, ctx: &dyn PlatformContext) {
        let window = ctx
            .create_window(WindowAttributes {
                title: "Lux".to_string(),
                inner_size: Some(LogicalSize::new(1000.0, 700.0)),
                ..Default::default()
            })
            .expect("failed to create the main window");
        window.set_ime_allowed(true);
        let window: &'static dyn PlatformWindow = Box::leak(window);

        let size = window.inner_size();
        self.set_display_scale(window.scale_factor());
        let gpu = pollster::block_on(GpuContext::new(None))
            .expect("a real GPU adapter must be available to run the Lux desktop app");
        let surface = Surface::new(&gpu.instance, &gpu.adapter, &gpu.device, window, size)
            .expect("failed to create the window's wgpu surface");
        let format = surface.config().format;
        let quads = QuadPipeline::new(&gpu.device, format);
        let text = TextPipeline::new(&gpu.device, format, ATLAS_SIDE);
        quads.set_viewport_size(&gpu.queue, size.width, size.height);
        text.set_globals(
            &gpu.queue,
            size.width,
            size.height,
            color(Theme::LUX_DARK.background),
        );

        self.viewport.width = size.width as f32;
        self.viewport.height = (size.height as f32 - self.tab_bar_height()).max(1.0);

        let id = window.id();
        self.gpu_window = Some(GpuWindow {
            window,
            id,
            gpu,
            surface,
            quads,
            text,
        });
        self.scheduler.invalidate_all();
        window.request_redraw();
    }

    pub(crate) fn resize(&mut self, size: PhysicalSize<u32>) {
        self.presentation
            .set_hidden(size.width == 0 || size.height == 0);
        let tab_bar_height = self.tab_bar_height();
        let Some(gpu_window) = &mut self.gpu_window else {
            return;
        };
        gpu_window.surface.resize(&gpu_window.gpu.device, size);
        gpu_window
            .quads
            .set_viewport_size(&gpu_window.gpu.queue, size.width, size.height);
        gpu_window.text.set_globals(
            &gpu_window.gpu.queue,
            size.width,
            size.height,
            color(Theme::LUX_DARK.background),
        );
        self.viewport.width = size.width as f32;
        self.viewport.height = (size.height as f32 - tab_bar_height).max(1.0);
        self.scheduler.invalidate_all();
        gpu_window.window.request_redraw();
    }

    pub(crate) fn scroll(&mut self, delta: ScrollDelta) {
        let dy = match delta {
            ScrollDelta::Lines { y, .. } => y * LINE_HEIGHT * self.scale_factor,
            ScrollDelta::Pixels(p) => p.y as f32,
        };
        self.viewport.scroll_y = (self.viewport.scroll_y - dy).max(0.0);
        self.scheduler.invalidate_all();
        if let Some(gpu_window) = &self.gpu_window {
            gpu_window.window.request_redraw();
        }
    }

    /// Paints one real frame: shapes only the visible lines (virtualized,
    /// same as `EditorView`'s own tests verify), rasterizes every visible
    /// glyph through the real atlas, and draws the background, selection,
    /// cursor, column rulers, indent guides, and — when IME composition is
    /// in progress — the real inline preedit text at the actual cursor
    /// position, all in one real `wgpu` submit + present.
    pub(crate) fn redraw(&mut self, ctx: &dyn PlatformContext) {
        if !self.presentation.ready(Instant::now()) {
            return;
        }
        let tab_bar_height = self.tab_bar_height();
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
            atlas,
            atlas_dirty,
            scale_cx,
            chrome,
            scale_factor,
            presentation,
            syntax,
            documents,
            ..
        } = self;
        let Some(gpu_window) = gpu_window.as_mut() else {
            return;
        };
        let font = *font;
        let font_size = FONT_SIZE * *scale_factor;
        let line_height = LINE_HEIGHT * *scale_factor;
        let stroke = *scale_factor;

        let elapsed = Instant::now().duration_since(*start);
        let frame = editor_view.build_frame(buffer, cursors, viewport, elapsed);
        let text_left = frame.gutter_width + TEXT_LEFT_PADDING * *scale_factor;

        let mut glyph_instances = Vec::new();
        let foreground = color(chrome.theme.foreground);
        for line in &frame.lines {
            let y = editor_top + line.index as f32 * line_height - viewport.scroll_y;
            let line_start = buffer.line_start_byte(line.index).unwrap_or(0);
            let line_len = buffer.line_content(line.index).map_or(0, |c| c.len());
            let line_end = line_start + line_len;
            let line_tokens = if let Some(syntax) = syntax.as_ref()
                && let Some(doc) = &syntax.document
            {
                doc.highlight_range(&syntax.source, line_start..line_end)
            } else {
                Vec::new()
            };

            for cluster in line.layout.clusters() {
                let abs_byte = line_start + cluster.byte_range.start;
                let glyph_color = if let Some(syntax) = syntax.as_ref()
                    && let Some(token) = line_tokens
                        .iter()
                        .find(|t| t.start_byte <= abs_byte && abs_byte < t.end_byte)
                {
                    syntax_color(syntax.theme.color_for_token(token))
                } else {
                    foreground
                };
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    font_size,
                    &cluster,
                    text_left - viewport.scroll_x,
                    y + line.layout.baseline(),
                    glyph_color,
                    &mut glyph_instances,
                );
            }
        }

        let mut quad_instances = vec![
            QuadInstance {
                rect: [0.0, 0.0, viewport.width, tab_bar_height],
                color: [0.055, 0.055, 0.075, 1.0],
            },
            QuadInstance {
                rect: [0.0, tab_bar_height - stroke, viewport.width, stroke],
                color: [0.15, 0.15, 0.20, 1.0],
            },
            QuadInstance {
                rect: [0.0, editor_top, viewport.width, viewport.height],
                color: color(chrome.theme.background),
            },
        ];

        let tab_font_size = font_size * 0.82;
        let family = editor_view.family().to_string();
        let tab_style = lux_render::LineStyle {
            family: &family,
            font_size: tab_font_size,
            ligatures: false,
        };
        let close_btn_w = crate::app::TAB_CLOSE_BTN_WIDTH * *scale_factor;
        for (i, tab) in documents.tabs.tabs().iter().enumerate() {
            let tab_x = i as f32 * tab_width;
            let is_active = Some(tab.id) == documents.active_tab;
            let tab_bg = if is_active {
                color(chrome.theme.background)
            } else {
                [0.075, 0.075, 0.10, 1.0]
            };
            quad_instances.push(QuadInstance {
                rect: [tab_x, 0.0, tab_width, tab_bar_height - stroke],
                color: tab_bg,
            });
            if is_active {
                quad_instances.push(QuadInstance {
                    rect: [tab_x, 0.0, tab_width, 2.0 * stroke],
                    color: color(chrome.theme.accent),
                });
            }
            quad_instances.push(QuadInstance {
                rect: [
                    tab_x + tab_width - stroke,
                    0.0,
                    stroke,
                    tab_bar_height - stroke,
                ],
                color: [0.15, 0.15, 0.20, 1.0],
            });

            let is_tab_dirty = if is_active {
                is_current_dirty
            } else {
                documents
                    .entries
                    .iter()
                    .find(|e| e.tab_id == tab.id)
                    .is_some_and(|e| e.is_dirty())
            };

            let title_color = if is_active {
                foreground
            } else {
                color(chrome.theme.muted)
            };

            let max_title_w = (tab_width - close_btn_w - 18.0 * stroke).max(0.0);
            let layout = editor_view
                .text_layout_mut()
                .layout_line(&tab_style, &tab.title, &[]);
            let title_y = (tab_bar_height - tab_font_size) * 0.5;
            for cluster in layout.clusters() {
                if cluster.x + 8.0 * stroke > max_title_w {
                    break;
                }
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    tab_font_size,
                    &cluster,
                    tab_x + 8.0 * stroke,
                    title_y + layout.baseline(),
                    title_color,
                    &mut glyph_instances,
                );
            }

            if is_tab_dirty {
                let dot_size = 5.0 * stroke;
                let dot_x = tab_x + tab_width - close_btn_w - dot_size - 2.0 * stroke;
                let dot_y = (tab_bar_height - dot_size) * 0.5;
                quad_instances.push(QuadInstance {
                    rect: [dot_x, dot_y, dot_size, dot_size],
                    color: color(chrome.theme.accent),
                });
            }

            let close_layout = editor_view
                .text_layout_mut()
                .layout_line(&tab_style, "×", &[]);
            let close_x = tab_x + tab_width - close_btn_w * 0.7;
            for cluster in close_layout.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    tab_font_size,
                    &cluster,
                    close_x,
                    title_y + close_layout.baseline(),
                    color(chrome.theme.muted),
                    &mut glyph_instances,
                );
            }
        }

        for ruler_x in &frame.column_ruler_x {
            quad_instances.push(QuadInstance {
                rect: [
                    text_left + ruler_x - viewport.scroll_x,
                    editor_top,
                    stroke,
                    viewport.height,
                ],
                color: color(chrome.theme.muted),
            });
        }
        for line in &frame.lines {
            let y = editor_top + line.index as f32 * line_height - viewport.scroll_y;
            for guide_x in &line.indent_guides {
                quad_instances.push(QuadInstance {
                    rect: [
                        text_left + guide_x - viewport.scroll_x,
                        y,
                        stroke,
                        line_height,
                    ],
                    color: color(chrome.theme.muted),
                });
            }
        }
        for selection in &frame.selections {
            let y = editor_top + selection.line as f32 * line_height - viewport.scroll_y;
            let width = (selection.x_end - selection.x_start).max(1.0);
            quad_instances.push(QuadInstance {
                rect: [
                    text_left + selection.x_start - viewport.scroll_x,
                    y,
                    width,
                    line_height,
                ],
                color: color(chrome.theme.selection),
            });
        }

        let mut cursor_screen = None;
        for cursor_glyph in &frame.cursors {
            let x = text_left + cursor_glyph.x - viewport.scroll_x;
            let y = editor_top + cursor_glyph.line as f32 * line_height - viewport.scroll_y;
            if cursor_glyph.primary {
                cursor_screen = Some((x, y));
            }
            if cursor_glyph.visible {
                quad_instances.push(QuadInstance {
                    rect: [x, y, 2.0 * stroke, line_height],
                    color: color(chrome.theme.accent),
                });
            }
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
                    atlas,
                    atlas_dirty,
                    scale_cx,
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
            quad_instances.push(QuadInstance {
                rect: [
                    x,
                    y + line_height - 2.0 * stroke,
                    layout.width().max(stroke),
                    2.0 * stroke,
                ],
                color: color(chrome.theme.accent),
            });
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

        if *atlas_dirty {
            gpu_window.text.upload_atlas(&gpu_window.gpu.queue, atlas);
            *atlas_dirty = false;
        }

        let outcome = paint(
            gpu_window,
            &quad_instances,
            &glyph_instances,
            atlas,
            color(chrome.theme.background),
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
    }
}
