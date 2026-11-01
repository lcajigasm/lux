//! Low-level rendering helpers, glyph caching, and wgpu render-pass submission.

use std::cell::Cell;

use swash::FontRef;
use wgpu::CurrentSurfaceTexture;

use crate::app::{ATLAS_SIDE, GpuWindow};
use crate::presentation::Outcome;
use lux_render::{
    GlyphAtlas, GlyphInstance, GlyphKey, QuadInstance, QuadPipeline, Surface, TextPipeline,
    TextRenderingMode,
};

thread_local! {
    /// HiDPI frames rasterize grayscale masks. The quad pass has already
    /// drawn each surface, so the mask composites against that real color
    /// instead of one editor-background uniform.
    static GRAYSCALE_TEXT: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn set_hidpi_grayscale(scale_factor: f32) {
    GRAYSCALE_TEXT.with(|cell| cell.set(scale_factor > 1.0));
}

thread_local! {
    static ACTIVE_THEME: Cell<lux_ui::Theme> = const { Cell::new(lux_ui::Theme::MONOKAI) };
}

/// Panels drawn outside the editor pass (terminal, LSP, git) read this.
/// `redraw` sets it from the live theme before those passes run.
pub(crate) fn set_active_theme(theme: lux_ui::Theme) {
    ACTIVE_THEME.with(|cell| cell.set(theme));
}

pub(crate) fn active_theme() -> lux_ui::Theme {
    ACTIVE_THEME.with(|cell| cell.get())
}

fn text_mode() -> TextRenderingMode {
    if GRAYSCALE_TEXT.with(|cell| cell.get()) {
        TextRenderingMode::Grayscale
    } else {
        TextRenderingMode::Subpixel
    }
}

#[allow(clippy::too_many_arguments)]
fn push_one_glyph(
    atlas: &mut lux_render::GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut swash::scale::ScaleContext,
    font: FontRef,
    font_size: f32,
    glyph_id: u32,
    x: f32,
    y_up: f32,
    origin_x: f32,
    origin_y: f32,
    color: [f32; 4],
    out: &mut Vec<GlyphInstance>,
) {
    let placed_x = origin_x + x;
    let key = GlyphKey::new(
        font.key.value(),
        glyph_id as u16,
        font_size,
        placed_x.fract(),
        text_mode(),
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
            placed_x.floor() + entry.left as f32,
            origin_y.round() - y_up - entry.top as f32,
            entry.width as f32,
            entry.height as f32,
        ],
        uv,
        color,
        entry.kind,
    ));
}

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
    if let Some(glyph_id) = cluster.glyph_id {
        push_one_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            glyph_id,
            cluster.x + cluster.glyph_dx,
            cluster.glyph_dy,
            origin_x,
            origin_y,
            color,
            out,
        );
    }
    for &(glyph_id, dx, dy) in &cluster.extra_glyphs {
        push_one_glyph(
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            font_size,
            glyph_id,
            cluster.x + dx,
            dy,
            origin_x,
            origin_y,
            color,
            out,
        );
    }
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

/// Byte ranges in `haystack` that a case-insensitive subsequence `needle` hits.
pub(crate) fn fuzzy_spans(haystack: &str, needle: &str) -> Vec<std::ops::Range<usize>> {
    let mut spans = Vec::new();
    if needle.trim().is_empty() {
        return spans;
    }
    let mut rest = haystack;
    let mut base = 0usize;
    for ch in needle.chars() {
        if ch.is_whitespace() {
            continue;
        }
        let lower = ch.to_lowercase().next().unwrap_or(ch);
        let Some((index, matched)) = rest
            .char_indices()
            .find(|(_, candidate)| candidate.to_lowercase().next().unwrap_or(*candidate) == lower)
        else {
            break;
        };
        let start = base + index;
        let len = matched.len_utf8();
        spans.push(start..start + len);
        let next = index + len;
        base += next;
        rest = &rest[next..];
    }
    spans
}

pub(crate) fn span_covers(
    spans: &[std::ops::Range<usize>],
    range: &std::ops::Range<usize>,
) -> bool {
    spans
        .iter()
        .any(|span| span.start < range.end && range.start < span.end)
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

#[cfg(test)]
mod tests {
    use super::{fuzzy_spans, span_covers};

    #[test]
    fn fuzzy_spans_cover_a_case_insensitive_subsequence() {
        let spans = fuzzy_spans("Goto Line", "gl");
        assert!(span_covers(&spans, &(0..1)), "{spans:?}");
        assert!(span_covers(&spans, &(5..6)), "{spans:?}");
        assert!(!span_covers(&spans, &(1..2)), "{spans:?}");
    }
}
