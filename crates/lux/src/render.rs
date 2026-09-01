//! Frame drawing: window/GPU setup and the actual per-frame draw calls.
//! Kept apart from `crate::app`'s event handling only to stay under the
//! 600-line file limit — this is still `impl App`, just in its own file.

use std::time::Instant;

use dpi::{LogicalSize, PhysicalSize};
use lux_platform::{PlatformContext, PlatformWindow, ScrollDelta, WindowAttributes};
use lux_render::{
    GlyphInstance, GpuContext, QuadInstance, QuadPipeline, Surface, TextPipeline,
};
use lux_ui::Theme;
use wgpu::CurrentSurfaceTexture;

use crate::tab_session::{language_label, TAB_STRIP_HEIGHT, STATUS_BAR_HEIGHT};
use lux_ui::status_bar;
use crate::glyph_draw::{color, push_glyph};
use crate::app::{
    ATLAS_SIDE, App, BLINK_INTERVAL, GpuWindow, TEXT_LEFT_PADDING,
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
        self.viewport.height = size.height as f32;

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
        self.viewport.height = size.height as f32;
        self.scheduler.invalidate_all();
        gpu_window.window.request_redraw();
    }

    pub(crate) fn scroll(&mut self, delta: ScrollDelta) {
        let line_height = self.editor_view.metrics().line_height;
        let dy = match delta {
            ScrollDelta::Lines { y, .. } => y * line_height,
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
            syntax,
            path,
            dirty,
            tabs,
            ..
        } = self;
        let Some(gpu_window) = gpu_window.as_mut() else {
            return;
        };
        let line_height = editor_view.metrics().line_height;
        let font_size = editor_view.metrics().font_size;
        let font = *font;
        let window_w = viewport.width;
        let window_h = viewport.height;
        let editor_top = TAB_STRIP_HEIGHT;
        let editor_h = (window_h - TAB_STRIP_HEIGHT - STATUS_BAR_HEIGHT).max(1.0);
        // Virtually shrink the viewport so virtualization matches the editor pane.
        viewport.height = editor_h;

        let elapsed = Instant::now().duration_since(*start);
        let frame = editor_view.build_frame(buffer, cursors, viewport, elapsed);
        viewport.height = window_h;
        let text_left = frame.gutter_width + TEXT_LEFT_PADDING;

        let mut glyph_instances = Vec::new();
        let foreground = color(chrome.theme.foreground);
        for line in &frame.lines {
            let y = editor_top + line.index as f32 * line_height - viewport.scroll_y;
            for cluster in line.layout.clusters() {
                let glyph_color = syntax
                    .as_ref()
                    .and_then(|hl| {
                        let abs = line.byte_start + cluster.byte_range.start;
                        hl.color_at(abs)
                    })
                    .map(crate::syntax::to_rgba)
                    .unwrap_or(foreground);
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    &cluster,
                    text_left - viewport.scroll_x,
                    y,
                    font_size,
                    glyph_color,
                    &mut glyph_instances,
                );
            }
        }

        let mut quad_instances = vec![QuadInstance {
            rect: [0.0, 0.0, viewport.width, viewport.height],
            color: color(chrome.theme.background),
        }];
        for ruler_x in &frame.column_ruler_x {
            quad_instances.push(QuadInstance {
                rect: [
                    text_left + ruler_x - viewport.scroll_x,
                    editor_top,
                    1.0,
                    editor_h,
                ],
                color: color(chrome.theme.muted),
            });
        }
        for line in &frame.lines {
            let y = editor_top + line.index as f32 * line_height - viewport.scroll_y;
            for guide_x in &line.indent_guides {
                quad_instances.push(QuadInstance {
                    rect: [text_left + guide_x - viewport.scroll_x, y, 1.0, line_height],
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
            cursor_screen = Some((x, y));
            if cursor_glyph.visible {
                quad_instances.push(QuadInstance {
                    rect: [x, y, 2.0, line_height],
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
                    &cluster,
                    x,
                    y,
                    font_size,
                    foreground,
                    &mut glyph_instances,
                );
            }
            // A thin underline distinguishes in-progress composition from
            // already-committed text — the same visual convention every
            // desktop IME candidate UI already uses.
            quad_instances.push(QuadInstance {
                rect: [x, y + line_height - 2.0, layout.width().max(1.0), 2.0],
                color: color(chrome.theme.accent),
            });
            gpu_window.window.set_ime_cursor_area(
                dpi::PhysicalPosition::new(x as i32, y as i32),
                dpi::PhysicalSize::new(layout.width().max(1.0) as u32, line_height as u32),
            );
        } else if let Some((x, y)) = cursor_screen {
            gpu_window.window.set_ime_cursor_area(
                dpi::PhysicalPosition::new(x as i32, y as i32),
                dpi::PhysicalSize::new(2, line_height as u32),
            );
        }

        let status = status_bar::build(
            buffer,
            cursors.primary().head,
            &language_label(path.as_deref()),
        );
        crate::chrome_paint::paint_tab_strip(
            chrome,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            tabs.bar(),
            window_w,
            &mut quad_instances,
            &mut glyph_instances,
        );
        crate::chrome_paint::paint_status_bar(
            chrome,
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            &status,
            window_w,
            window_h,
            &mut quad_instances,
            &mut glyph_instances,
        );
        let _ = dirty; // dirty marker is already reflected in tab titles

        if *atlas_dirty {
            gpu_window.text.upload_atlas(&gpu_window.gpu.queue, atlas);
            *atlas_dirty = false;
        }

        let _ = scheduler.take_dirty_regions();
        paint(gpu_window, &quad_instances, &glyph_instances);

        if chrome.motion.cursor_blinks() {
            let phase = elapsed.as_millis() / BLINK_INTERVAL.as_millis().max(1);
            let next_toggle = *start + BLINK_INTERVAL * (phase as u32 + 1);
            scheduler.request_repaint_at(next_toggle);
            ctx.request_wakeup(next_toggle);
        }
    }
}

fn paint(gpu_window: &mut GpuWindow, quads: &[QuadInstance], glyphs: &[GlyphInstance]) {
    let current = gpu_window.surface.acquire_frame();
    let texture = match current {
        CurrentSurfaceTexture::Success(t) | CurrentSurfaceTexture::Suboptimal(t) => t,
        CurrentSurfaceTexture::Timeout | CurrentSurfaceTexture::Occluded => return,
        CurrentSurfaceTexture::Outdated => {
            let size = PhysicalSize::new(
                gpu_window.surface.config().width,
                gpu_window.surface.config().height,
            );
            gpu_window.surface.resize(&gpu_window.gpu.device, size);
            gpu_window.window.request_redraw();
            return;
        }
        // Real gap: recovering from a lost surface needs rebuilding a whole
        // new `wgpu::Surface` (this crate's `Surface::new` again), not
        // implemented in this first pass — flagged, not silently ignored.
        CurrentSurfaceTexture::Lost | CurrentSurfaceTexture::Validation => return,
    };
    let view = texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = gpu_window
        .gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("lux frame"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        gpu_window.quads.draw(
            &gpu_window.gpu.device,
            &gpu_window.gpu.queue,
            &mut pass,
            quads,
        );
        gpu_window.text.draw(
            &gpu_window.gpu.device,
            &gpu_window.gpu.queue,
            &mut pass,
            glyphs,
        );
    }
    gpu_window.window.pre_present_notify();
    gpu_window.gpu.queue.submit(Some(encoder.finish()));
    gpu_window.gpu.queue.present(texture);
}
