//! Frame drawing: window/GPU setup and the actual per-frame draw calls.
//! Kept apart from `crate::app`'s event handling only to stay under the
//! 600-line file limit — this is still `impl App`, just in its own file.

use std::time::Instant;

use dpi::{LogicalSize, PhysicalSize};
use lux_platform::{PlatformContext, PlatformWindow, ScrollDelta, WindowAttributes};
use lux_render::{
    GlyphInstance, GlyphKey, GpuContext, QuadInstance, QuadPipeline, Surface, TextPipeline,
    TextRenderingMode,
};
use lux_ui::Theme;
use swash::FontRef;
use wgpu::CurrentSurfaceTexture;

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
        let dy = match delta {
            ScrollDelta::Lines { y, .. } => y * LINE_HEIGHT,
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
            ..
        } = self;
        let Some(gpu_window) = gpu_window.as_mut() else {
            return;
        };
        let font = *font;

        let elapsed = Instant::now().duration_since(*start);
        let frame = editor_view.build_frame(buffer, cursors, viewport, elapsed);
        let text_left = frame.gutter_width + TEXT_LEFT_PADDING;

        let mut glyph_instances = Vec::new();
        let foreground = color(chrome.theme.foreground);
        for line in &frame.lines {
            let y = line.index as f32 * LINE_HEIGHT - viewport.scroll_y;
            for cluster in line.layout.clusters() {
                push_glyph(
                    atlas,
                    atlas_dirty,
                    scale_cx,
                    font,
                    &cluster,
                    text_left - viewport.scroll_x,
                    y,
                    foreground,
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
                    0.0,
                    1.0,
                    viewport.height,
                ],
                color: color(chrome.theme.muted),
            });
        }
        for line in &frame.lines {
            let y = line.index as f32 * LINE_HEIGHT - viewport.scroll_y;
            for guide_x in &line.indent_guides {
                quad_instances.push(QuadInstance {
                    rect: [text_left + guide_x - viewport.scroll_x, y, 1.0, LINE_HEIGHT],
                    color: color(chrome.theme.muted),
                });
            }
        }
        for selection in &frame.selections {
            let y = selection.line as f32 * LINE_HEIGHT - viewport.scroll_y;
            let width = (selection.x_end - selection.x_start).max(1.0);
            quad_instances.push(QuadInstance {
                rect: [
                    text_left + selection.x_start - viewport.scroll_x,
                    y,
                    width,
                    LINE_HEIGHT,
                ],
                color: color(chrome.theme.selection),
            });
        }

        let mut cursor_screen = None;
        for cursor_glyph in &frame.cursors {
            let x = text_left + cursor_glyph.x - viewport.scroll_x;
            let y = cursor_glyph.line as f32 * LINE_HEIGHT - viewport.scroll_y;
            cursor_screen = Some((x, y));
            if cursor_glyph.visible {
                quad_instances.push(QuadInstance {
                    rect: [x, y, 2.0, LINE_HEIGHT],
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
                font_size: FONT_SIZE,
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
                    foreground,
                    &mut glyph_instances,
                );
            }
            // A thin underline distinguishes in-progress composition from
            // already-committed text — the same visual convention every
            // desktop IME candidate UI already uses.
            quad_instances.push(QuadInstance {
                rect: [x, y + LINE_HEIGHT - 2.0, layout.width().max(1.0), 2.0],
                color: color(chrome.theme.accent),
            });
            gpu_window.window.set_ime_cursor_area(
                dpi::PhysicalPosition::new(x as i32, y as i32),
                dpi::PhysicalSize::new(layout.width().max(1.0) as u32, LINE_HEIGHT as u32),
            );
        } else if let Some((x, y)) = cursor_screen {
            gpu_window.window.set_ime_cursor_area(
                dpi::PhysicalPosition::new(x as i32, y as i32),
                dpi::PhysicalSize::new(2, LINE_HEIGHT as u32),
            );
        }

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

#[allow(clippy::too_many_arguments)]
fn push_glyph(
    atlas: &mut lux_render::GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut swash::scale::ScaleContext,
    font: FontRef,
    cluster: &lux_render::ClusterGeometry,
    origin_x: f32,
    origin_y: f32,
    color: [f32; 4],
    out: &mut Vec<GlyphInstance>,
) {
    let Some(glyph_id) = cluster.glyph_id else {
        return;
    };
    // TrueType/OpenType glyph indices are defined as 16-bit by the font
    // format spec itself; `parley::Glyph::id` is a `u32` for API-level
    // headroom, but no real font can produce a value this cast would
    // truncate.
    let key = GlyphKey::new(
        font.key.value(),
        glyph_id as u16,
        FONT_SIZE,
        cluster.x.fract(),
        TextRenderingMode::Subpixel,
    );
    let was_cached = atlas.is_cached(&key);
    let Some(entry) = atlas.rasterize(scale_cx, font, key) else {
        return; // no visible ink (e.g. a space) — nothing to draw
    };
    if !was_cached {
        *atlas_dirty = true;
    }
    let side = ATLAS_SIDE as f32;
    let uv = [
        entry.atlas_x as f32 / side,
        entry.atlas_y as f32 / side,
        (entry.atlas_x + entry.width) as f32 / side,
        (entry.atlas_y + entry.height) as f32 / side,
    ];
    out.push(GlyphInstance::new(
        [
            origin_x + cluster.x,
            origin_y,
            entry.width as f32,
            entry.height as f32,
        ],
        uv,
        color,
        entry.kind,
    ));
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

/// `Theme::Color`'s straight sRGB-space bytes, normalized to `0.0..=1.0` —
/// matched consistently between the clear/background quad and the text
/// pipeline's `background_color` uniform (what subpixel compositing mixes
/// against), which is the only thing that actually has to agree; whether
/// that number is *technically* linear or sRGB gamma doesn't change the
/// internal consistency of that mix. A real gamma-correct pass is future
/// polish, not a correctness bug this app hits today.
fn color(c: lux_ui::Color) -> [f32; 4] {
    [
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        c.a as f32 / 255.0,
    ]
}
