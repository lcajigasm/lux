//! Low-level rendering helpers, glyph caching, and wgpu render-pass submission.

use swash::FontRef;
use wgpu::CurrentSurfaceTexture;

use crate::app::{ATLAS_SIDE, GpuWindow};
use crate::presentation::Outcome;
use lux_render::{
    GlyphAtlas, GlyphInstance, GlyphKey, QuadInstance, QuadPipeline, Surface, TextPipeline,
    TextRenderingMode,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn push_glyph(
    atlas: &mut lux_render::GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut swash::scale::ScaleContext,
    font: FontRef,
    font_size: f32,
    cluster: &lux_render::ClusterGeometry,
    origin_x: f32,
    origin_y: f32,
    color: [f32; 4],
    out: &mut Vec<GlyphInstance>,
) {
    let Some(glyph_id) = cluster.glyph_id else {
        return;
    };
    let key = GlyphKey::new(
        font.key.value(),
        glyph_id as u16,
        font_size,
        (origin_x + cluster.x).fract(),
        TextRenderingMode::Subpixel,
    );
    let was_cached = atlas.is_cached(&key);
    let Some(entry) = atlas.rasterize(scale_cx, font, key) else {
        return;
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
            (origin_x + cluster.x).floor() + entry.left as f32,
            origin_y.round() - entry.top as f32,
            entry.width as f32,
            entry.height as f32,
        ],
        uv,
        color,
        entry.kind,
    ));
}

pub(super) fn paint(
    gpu_window: &mut GpuWindow,
    quads: &[QuadInstance],
    glyphs: &[GlyphInstance],
    atlas: &GlyphAtlas,
    background: [f32; 4],
) -> Outcome {
    let current = gpu_window.surface.acquire_frame();
    let texture = match current {
        CurrentSurfaceTexture::Success(t) | CurrentSurfaceTexture::Suboptimal(t) => t,
        CurrentSurfaceTexture::Timeout => return Outcome::Retry,
        CurrentSurfaceTexture::Occluded => return Outcome::Hidden,
        CurrentSurfaceTexture::Outdated => {
            gpu_window.surface.reconfigure(&gpu_window.gpu.device);
            return Outcome::Retry;
        }
        CurrentSurfaceTexture::Lost => {
            let size = gpu_window.window.inner_size();
            let surface = match Surface::new(
                &gpu_window.gpu.instance,
                &gpu_window.gpu.adapter,
                &gpu_window.gpu.device,
                gpu_window.window,
                size,
            ) {
                Ok(surface) => surface,
                Err(error) => {
                    eprintln!("Lux: cannot recreate rendering surface: {error}");
                    return Outcome::Failed;
                }
            };
            let format = surface.config().format;
            gpu_window.surface = surface;
            gpu_window.quads = QuadPipeline::new(&gpu_window.gpu.device, format);
            gpu_window.text = TextPipeline::new(&gpu_window.gpu.device, format, ATLAS_SIDE);
            gpu_window
                .quads
                .set_viewport_size(&gpu_window.gpu.queue, size.width, size.height);
            gpu_window
                .text
                .set_globals(&gpu_window.gpu.queue, size.width, size.height, background);
            gpu_window.text.upload_atlas(&gpu_window.gpu.queue, atlas);
            return Outcome::Retry;
        }
        CurrentSurfaceTexture::Validation => {
            eprintln!("Lux: surface acquisition failed validation");
            return Outcome::Failed;
        }
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
    Outcome::Presented
}

pub(crate) fn color(c: lux_ui::Color) -> [f32; 4] {
    [
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        c.a as f32 / 255.0,
    ]
}

pub(super) fn syntax_color(c: lux_syntax::theme::Color) -> [f32; 4] {
    [
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        1.0,
    ]
}
