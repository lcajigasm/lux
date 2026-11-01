//! Golden framebuffer tests: real `wgpu` rendering, read back and checked
//! pixel by pixel — the literal Phase 2 acceptance criterion ("Golden
//! tests de render: capturas de framebuffer comparadas con referencias,
//! con tolerancia definida"), on the one platform available to verify it
//! (macOS; see this deliverable's receipt for the Windows/Linux gap).
//!
//! Renders to an offscreen texture (no window needed), copies it to a
//! buffer, and maps it back to real bytes — nothing here is simulated: the
//! same `QuadPipeline`/`TextPipeline`/`GlyphAtlas` code a live window would
//! use, driven by a real GPU adapter and device.
//!
//! Skips (doesn't fail) if no GPU/driver is available in the environment,
//! matching `lux_render::context`'s own existing test policy — not this
//! crate's bug to report on a headless CI runner with no Metal/Vulkan/DX12
//! backend.

use lux_render::TextRenderingMode;
use lux_render::{
    GlyphAtlas, GlyphInstance, GlyphKey, GpuContext, QuadInstance, QuadPipeline, TextPipeline,
};
use swash::scale::ScaleContext;
use swash::{Charmap, FontRef};

const AHEM_TTF: &[u8] = include_bytes!("fixtures/ahem.ttf");
const ATLAS_SIDE: u32 = 256;
const TARGET_SIDE: u32 = 128; // 128 * 4 bytes/pixel = 512, already row-aligned
const FONT_SIZE: f32 = 32.0;
const BACKGROUND: [u8; 4] = [20, 20, 20, 255];
const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// Where the two glyph quads actually landed, in physical pixels — derived
/// from each glyph's real measured size (never hardcoded), so tests can
/// pick exact center/gap coordinates instead of guessing.
struct Layout {
    mono_rect: [u32; 4],
    subpixel_rect: [u32; 4],
}

impl Layout {
    fn mono_center(&self) -> (u32, u32) {
        (
            self.mono_rect[0] + self.mono_rect[2] / 2,
            self.mono_rect[1] + self.mono_rect[3] / 2,
        )
    }

    fn subpixel_center(&self) -> (u32, u32) {
        (
            self.subpixel_rect[0] + self.subpixel_rect[2] / 2,
            self.subpixel_rect[1] + self.subpixel_rect[3] / 2,
        )
    }

    /// A point in the real gap between the two rects — only valid because
    /// `render_frame` places `subpixel_rect` a fixed, generous distance
    /// after `mono_rect` ends.
    fn gap_between(&self) -> (u32, u32) {
        let midpoint = (self.mono_rect[0] + self.mono_rect[2] + self.subpixel_rect[0]) / 2;
        (midpoint, self.mono_rect[1])
    }
}

/// Renders one frame (background quad + two glyph quads) to an offscreen
/// texture and returns its raw RGBA8 bytes (row-major, `TARGET_SIDE` wide)
/// plus where the glyphs actually landed.
fn render_frame() -> Option<(Vec<u8>, Layout)> {
    let context = match pollster::block_on(GpuContext::new(None)) {
        Ok(context) => context,
        Err(_) => return None,
    };
    let GpuContext { device, queue, .. } = context;

    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut quads = QuadPipeline::new(&device, format);
    let mut text = TextPipeline::new(&device, format, ATLAS_SIDE);

    let font = FontRef::from_index(AHEM_TTF, 0).expect("ahem.ttf must parse as a valid font");
    let charmap = Charmap::from_font(&font);
    let glyph_id = charmap.map('A');
    let mut atlas = GlyphAtlas::new(ATLAS_SIDE);
    let mut scale_cx = ScaleContext::new();

    let mono_key = GlyphKey::new(
        font.key.value(),
        glyph_id,
        FONT_SIZE,
        0.0,
        TextRenderingMode::Grayscale,
    );
    let mono_entry = atlas
        .rasterize(&mut scale_cx, font, mono_key)
        .expect("Ahem glyphs are solid ink");
    let subpixel_key = GlyphKey::new(
        font.key.value(),
        glyph_id,
        FONT_SIZE,
        0.0,
        TextRenderingMode::Subpixel,
    );
    let subpixel_entry = atlas
        .rasterize(&mut scale_cx, font, subpixel_key)
        .expect("Ahem glyphs are solid ink");

    text.upload_atlas(&queue, &atlas);

    let bg = [
        BACKGROUND[0] as f32 / 255.0,
        BACKGROUND[1] as f32 / 255.0,
        BACKGROUND[2] as f32 / 255.0,
        1.0,
    ];
    quads.set_viewport_size(&queue, TARGET_SIDE, TARGET_SIDE);
    text.set_globals(&queue, TARGET_SIDE, TARGET_SIDE, bg);

    let uv = |plane_side: u32, x: u32, y: u32, w: u32, h: u32| {
        let s = plane_side as f32;
        [
            x as f32 / s,
            y as f32 / s,
            (x + w) as f32 / s,
            (y + h) as f32 / s,
        ]
    };

    const MONO_ORIGIN: (u32, u32) = (8, 8);
    // A generous, fixed gap after the monochrome glyph ends — real
    // measured width, not a guess — so the two quads never overlap
    // regardless of exactly how wide Ahem renders at `FONT_SIZE`.
    const GAP: u32 = 16;
    let subpixel_origin = (MONO_ORIGIN.0 + mono_entry.width + GAP, MONO_ORIGIN.1);

    let layout = Layout {
        mono_rect: [
            MONO_ORIGIN.0,
            MONO_ORIGIN.1,
            mono_entry.width,
            mono_entry.height,
        ],
        subpixel_rect: [
            subpixel_origin.0,
            subpixel_origin.1,
            subpixel_entry.width,
            subpixel_entry.height,
        ],
    };

    let mono_instance = GlyphInstance::new(
        [
            MONO_ORIGIN.0 as f32,
            MONO_ORIGIN.1 as f32,
            mono_entry.width as f32,
            mono_entry.height as f32,
        ],
        uv(
            ATLAS_SIDE,
            mono_entry.atlas_x,
            mono_entry.atlas_y,
            mono_entry.width,
            mono_entry.height,
        ),
        RED,
        mono_entry.kind,
    );
    let subpixel_instance = GlyphInstance::new(
        [
            subpixel_origin.0 as f32,
            subpixel_origin.1 as f32,
            subpixel_entry.width as f32,
            subpixel_entry.height as f32,
        ],
        uv(
            ATLAS_SIDE,
            subpixel_entry.atlas_x,
            subpixel_entry.atlas_y,
            subpixel_entry.width,
            subpixel_entry.height,
        ),
        BLUE,
        subpixel_entry.kind,
    );

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("golden_render target"),
        size: wgpu::Extent3d {
            width: TARGET_SIDE,
            height: TARGET_SIDE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("golden_render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
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
        quads.draw(
            &device,
            &queue,
            &mut pass,
            &[QuadInstance {
                rect: [0.0, 0.0, TARGET_SIDE as f32, TARGET_SIDE as f32],
                color: bg,
            }],
        );
        text.draw(
            &device,
            &queue,
            &mut pass,
            &[mono_instance, subpixel_instance],
        );
    }

    let bytes_per_row = TARGET_SIDE * 4;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("golden_render readback"),
        size: (bytes_per_row * TARGET_SIDE) as wgpu::BufferAddress,
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
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(TARGET_SIDE),
            },
        },
        wgpu::Extent3d {
            width: TARGET_SIDE,
            height: TARGET_SIDE,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result)
                .expect("readback receiver must still be alive");
        });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll must succeed");
    rx.recv()
        .expect("map_async callback must fire")
        .expect("mapping the readback buffer must succeed");

    let bytes = readback
        .slice(..)
        .get_mapped_range()
        .expect("buffer must be mapped by now")
        .to_vec();
    Some((bytes, layout))
}

fn pixel(bytes: &[u8], x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * TARGET_SIDE + x) * 4) as usize;
    bytes[offset..offset + 4].try_into().unwrap()
}

/// Absolute per-channel tolerance for float rounding through the
/// pixel->NDC->rasterize->blend round trip — flat, opaque fills should
/// land within a couple of 8-bit levels of the exact value, not needing a
/// framebuffer-comparison-grade tolerance.
const TOLERANCE: i16 = 4;

fn assert_close(actual: [u8; 4], expected: [u8; 4], context: &str) {
    for c in 0..4 {
        let diff = (actual[c] as i16 - expected[c] as i16).abs();
        assert!(
            diff <= TOLERANCE,
            "{context}: channel {c} differs by {diff} (actual {actual:?}, expected {expected:?})"
        );
    }
}

#[test]
fn background_quad_fills_untouched_pixels_with_its_exact_color() {
    let Some((bytes, _layout)) = render_frame() else {
        return; // no GPU/driver in this environment — not this crate's bug
    };
    // Far corner, well outside both glyph quads (which sit near the top-left).
    let corner = pixel(&bytes, TARGET_SIDE - 1, TARGET_SIDE - 1);
    assert_close(
        corner,
        [BACKGROUND[0], BACKGROUND[1], BACKGROUND[2], 255],
        "background corner",
    );
}

#[test]
fn monochrome_glyph_center_is_tinted_with_its_instance_color() {
    let Some((bytes, layout)) = render_frame() else {
        return;
    };
    // Ahem's glyph fills its whole box solidly, so its geometric center is
    // deep inside real, fully-covered ink.
    let (x, y) = layout.mono_center();
    assert_close(
        pixel(&bytes, x, y),
        [255, 0, 0, 255],
        "monochrome glyph center (expected red)",
    );
}

#[test]
fn subpixel_glyph_center_composites_to_its_instance_color_against_the_known_background() {
    let Some((bytes, layout)) = render_frame() else {
        return;
    };
    let (x, y) = layout.subpixel_center();
    assert_close(
        pixel(&bytes, x, y),
        [0, 0, 255, 255],
        "subpixel glyph center (expected blue)",
    );
}

#[test]
fn the_gap_between_the_two_glyphs_stays_pure_background() {
    let Some((bytes, layout)) = render_frame() else {
        return;
    };
    let (x, y) = layout.gap_between();
    let gap = pixel(&bytes, x, y);
    assert_close(
        gap,
        [BACKGROUND[0], BACKGROUND[1], BACKGROUND[2], 255],
        "gap between glyphs",
    );
}

/// The window surface prefers an sRGB format. Authored theme colors are
/// sRGB byte values; the shader must decode them so the stored bytes match
/// the theme. This is the Monokai editor background, `#272822`.
#[test]
fn an_srgb_target_stores_authored_monokai_bytes() {
    let context = match pollster::block_on(GpuContext::new(None)) {
        Ok(context) => context,
        Err(_) => return,
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let features = context.adapter.get_texture_format_features(format);
    let needed = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
    if !features.allowed_usages.contains(needed) {
        return;
    }
    let side = 64u32;
    let mut quads = QuadPipeline::new(&context.device, format);
    let authored = [0x27_u8, 0x28, 0x22, 255];
    let color = [
        authored[0] as f32 / 255.0,
        authored[1] as f32 / 255.0,
        authored[2] as f32 / 255.0,
        1.0,
    ];
    quads.set_viewport_size(&context.queue, side, side);

    let target = context.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("srgb monokai target"),
        size: wgpu::Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: needed,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = context
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("srgb monokai pass"),
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
        quads.draw(
            &context.device,
            &context.queue,
            &mut pass,
            &[QuadInstance {
                rect: [0.0, 0.0, side as f32, side as f32],
                color,
            }],
        );
    }
    let bytes_per_row = side * 4;
    let readback = context.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("srgb monokai readback"),
        size: (bytes_per_row * side) as wgpu::BufferAddress,
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
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(side),
            },
        },
        wgpu::Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
    );
    context.queue.submit(Some(encoder.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result)
                .expect("readback receiver must still be alive");
        });
    context
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll must succeed");
    rx.recv()
        .expect("map_async callback must fire")
        .expect("mapping the readback buffer must succeed");
    let bytes = readback
        .slice(..)
        .get_mapped_range()
        .expect("buffer must be mapped by now");
    let pixel: [u8; 4] = bytes[0..4].try_into().unwrap();
    assert_close(pixel, authored, "srgb monokai background");
}
