//! One offscreen frame of the running renderer at a chosen scale factor.
//! The window capture on a 1× display cannot show Retina pixels. This path
//! builds the same quads and glyphs into an RGBA texture and writes a PNG.

use std::cell::RefCell;
use std::path::PathBuf;

use crate::app::{ATLAS_SIDE, App, GpuWindow};
use crate::presentation::Outcome;
use crate::render::paint::{self, DrawList};
use lux_platform::PlatformContext;
use lux_render::{GlyphAtlas, QuadPipeline, TextPipeline};

struct Request {
    path: PathBuf,
    scale: f32,
    written: bool,
}

pub(crate) struct Capture {
    width: u32,
    height: u32,
    path: PathBuf,
}

thread_local! {
    static REQUEST: RefCell<Option<Request>> = const { RefCell::new(None) };
    static PENDING: RefCell<Option<Capture>> = const { RefCell::new(None) };
}

pub(crate) struct FrameExtent {
    pub width: f32,
    pub height: f32,
    pub capture: Option<Capture>,
}

pub(crate) fn request(path: PathBuf, scale: f32) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        2.0
    };
    REQUEST.with(|slot| {
        *slot.borrow_mut() = Some(Request {
            path,
            scale,
            written: false,
        });
    });
}

pub(crate) fn frame_extent(window_width: u32, window_height: u32) -> FrameExtent {
    if let Some(capture) = PENDING.with(|slot| slot.borrow_mut().take()) {
        FrameExtent {
            width: capture.width as f32,
            height: capture.height as f32,
            capture: Some(capture),
        }
    } else {
        FrameExtent {
            width: window_width as f32,
            height: window_height as f32,
            capture: None,
        }
    }
}

pub(crate) fn present_or_capture(
    gpu_window: &mut GpuWindow,
    base: DrawList<'_>,
    overlay: DrawList<'_>,
    atlas: &GlyphAtlas,
    background: [f32; 4],
    capture: Option<Capture>,
) -> Outcome {
    let Some(capture) = capture else {
        return paint::paint(gpu_window, base, overlay, atlas, background);
    };
    match write_texture(gpu_window, base, overlay, atlas, background, &capture) {
        Ok(()) => eprintln!(
            "Lux: wrote scale snapshot {} ({}x{})",
            capture.path.display(),
            capture.width,
            capture.height
        ),
        Err(error) => eprintln!("Lux: scale snapshot failed: {error}"),
    }
    Outcome::Presented
}

/// After a presented frame has settled, render the same scene again at the
/// requested scale into a PNG. The on-screen window is unchanged.
pub(crate) fn capture_settled(app: &mut App, ctx: &dyn PlatformContext) {
    let Some((path, scale)) = REQUEST.with(|slot| {
        let req = slot.borrow();
        let req = req.as_ref()?;
        if req.written {
            return None;
        }
        Some((req.path.clone(), req.scale))
    }) else {
        return;
    };
    if !settled(app) {
        return;
    }
    if !app.presentation.ready(std::time::Instant::now()) {
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
        return;
    }
    let Some(gpu) = app.gpu_window.as_ref() else {
        return;
    };
    let physical = gpu.window.inner_size();
    let current = app.scale_factor.max(0.01);
    let logical_w = physical.width as f32 / current;
    let logical_h = physical.height as f32 / current;
    let width = (logical_w * scale).round().max(1.0) as u32;
    let height = (logical_h * scale).round().max(1.0) as u32;

    let saved_scale = app.scale_factor;
    let saved_width = app.viewport.width;
    let saved_height = app.viewport.height;
    let saved_scroll_x = app.viewport.scroll_x;
    let saved_scroll_y = app.viewport.scroll_y;
    app.set_display_scale(f64::from(scale));
    let tab = app.tab_bar_height();
    let status = app.status_bar_height();
    let chrome = app.sidebar_width() + app.minimap_width();
    app.viewport.width = (width as f32 - chrome).max(1.0);
    app.viewport.height = (height as f32 - tab - status).max(1.0);
    PENDING.with(|slot| {
        *slot.borrow_mut() = Some(Capture {
            width,
            height,
            path,
        });
    });
    app.redraw(ctx);
    app.set_display_scale(f64::from(saved_scale));
    app.viewport.width = saved_width;
    app.viewport.height = saved_height;
    app.viewport.scroll_x = saved_scroll_x;
    app.viewport.scroll_y = saved_scroll_y;
    REQUEST.with(|slot| {
        if let Some(req) = slot.borrow_mut().as_mut() {
            req.written = true;
        }
    });
    if let Some(gpu) = &app.gpu_window {
        gpu.window.request_redraw();
    }
}

fn settled(app: &App) -> bool {
    if crate::document::open_queue::busy(app) || !app.documents.open_queue.is_empty() {
        return false;
    }
    let now = std::time::Instant::now();
    let reduced = app.chrome.motion.reduced;
    let moving = |motion: &crate::overlay_motion::OverlayMotion| motion.animating(now, reduced);
    if app
        .ui
        .command_palette
        .as_ref()
        .is_some_and(|p| moving(&p.motion))
    {
        return false;
    }
    if app
        .ui
        .goto_anything
        .as_ref()
        .is_some_and(|g| moving(&g.motion))
    {
        return false;
    }
    if app
        .ui
        .theme_picker
        .as_ref()
        .is_some_and(|p| moving(&p.motion))
    {
        return false;
    }
    true
}

fn write_texture(
    gpu_window: &mut GpuWindow,
    base: DrawList<'_>,
    overlay: DrawList<'_>,
    atlas: &GlyphAtlas,
    background: [f32; 4],
    capture: &Capture,
) -> Result<(), String> {
    let device = &gpu_window.gpu.device;
    let queue = &gpu_window.gpu.queue;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut quads_pipe = QuadPipeline::new(device, format);
    let mut text = TextPipeline::new(device, format, ATLAS_SIDE);
    quads_pipe.set_viewport_size(queue, capture.width, capture.height);
    text.set_globals(queue, capture.width, capture.height, background);
    text.upload_atlas(queue, atlas);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("lux scale snapshot"),
        size: wgpu::Extent3d {
            width: capture.width,
            height: capture.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("lux scale snapshot"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("lux scale snapshot"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: background[0] as f64,
                        g: background[1] as f64,
                        b: background[2] as f64,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        quads_pipe.draw(device, queue, &mut pass, base.quads);
        text.draw(device, queue, &mut pass, base.glyphs);
    }
    // Same instance-buffer rule as the window: submit the editor before the
    // selector draw replaces it. One submit kept only the clear color.
    if !overlay.quads.is_empty() || !overlay.glyphs.is_empty() {
        queue.submit(Some(encoder.finish()));
        encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("lux scale snapshot overlay"),
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("lux scale snapshot overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        quads_pipe.draw(device, queue, &mut pass, overlay.quads);
        text.draw(device, queue, &mut pass, overlay.glyphs);
    }

    let tight = capture.width * 4;
    let padded = tight.div_ceil(256) * 256;
    let buffer_size = padded as u64 * capture.height as u64;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("lux scale snapshot readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(capture.height),
            },
        },
        target.size(),
    );
    queue.submit(Some(encoder.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| error.to_string())?;
    rx.recv()
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let mapped = readback
        .slice(..)
        .get_mapped_range()
        .map_err(|error| error.to_string())?;
    let tight = capture.width * 4;
    let mut rgba = Vec::with_capacity(tight as usize * capture.height as usize);
    for row in 0..capture.height {
        let start = row as usize * padded as usize;
        rgba.extend_from_slice(&mapped[start..start + tight as usize]);
    }
    drop(mapped);
    readback.unmap();
    let png = encode_png(capture.width, capture.height, &rgba);
    if let Some(parent) = capture.path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(&capture.path, png).map_err(|error| error.to_string())
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((width as usize * 4 + 1) * height as usize);
    let stride = width as usize * 4;
    for row in 0..height as usize {
        raw.push(0);
        raw.extend_from_slice(&rgba[row * stride..(row + 1) * stride]);
    }
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend(width.to_be_bytes());
    ihdr.extend(height.to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]);
    push_chunk(&mut out, b"IHDR", &ihdr);
    push_chunk(&mut out, b"IDAT", &zlib_store(&raw));
    push_chunk(&mut out, b"IEND", &[]);
    out
}

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes());
    out.extend(kind);
    out.extend(data);
    let crc = crc32(&out[out.len() - 4 - data.len()..]);
    out.extend(crc.to_be_bytes());
}

fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut zlib = vec![0x78, 0x01];
    let mut rest = data;
    while !rest.is_empty() {
        let take = rest.len().min(65535);
        let last = take == rest.len();
        zlib.push(if last { 1 } else { 0 });
        zlib.extend((take as u16).to_le_bytes());
        zlib.extend((!take as u16).to_le_bytes());
        zlib.extend(&rest[..take]);
        rest = &rest[take..];
    }
    zlib.extend(adler32(data).to_be_bytes());
    zlib
}

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for byte in data {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_one_pixel_png_names_its_size() {
        let png = encode_png(1, 1, &[255, 0, 0, 255]);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[16..20], &1u32.to_be_bytes());
        assert_eq!(&png[20..24], &1u32.to_be_bytes());
        assert!(png.windows(4).any(|chunk| chunk == b"IEND"));
    }
}
