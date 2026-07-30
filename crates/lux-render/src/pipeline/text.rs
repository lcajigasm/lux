//! Instanced, textured glyph quads sampling [`crate::GlyphAtlas`]'s three
//! planes.
//!
//! Subpixel LCD compositing needs the destination color to be known in
//! advance — that's an inherent property of subpixel AA, not something
//! specific to this renderer (it's why terminals with transparent
//! backgrounds fall back to grayscale AA). Rather than dual-source
//! blending (limited backend support in `wgpu`), the fragment shader mixes
//! the subpixel mask against a `background_color` uniform directly and
//! outputs a fully opaque color; regular alpha blending with alpha = 1.0
//! degenerates to a plain replace, so one blend state serves all three
//! glyph kinds. See `text.wgsl` for the actual per-kind compositing.

use wgpu::util::DeviceExt;

use crate::GlyphAtlas;
use crate::glyph_atlas::GlyphKind;

const SHADER_SRC: &str = include_str!("text.wgsl");

/// One glyph quad: screen rect, atlas UV rect (normalized 0..1), tint, and
/// which plane to sample. Callers (eventually `lux-ui`) build these from
/// `lux_render::text_layout::ClusterGeometry` plus whatever
/// `GlyphAtlas::rasterize` returned for each cluster.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlyphInstance {
    pub rect: [f32; 4],
    pub uv_rect: [f32; 4],
    pub color: [f32; 4],
    pub kind: u32,
    _padding: [u32; 3],
}

impl GlyphInstance {
    pub fn new(rect: [f32; 4], uv_rect: [f32; 4], color: [f32; 4], kind: GlyphKind) -> Self {
        let kind = match kind {
            GlyphKind::Monochrome => 0,
            GlyphKind::Polychrome => 1,
            GlyphKind::Subpixel => 2,
        };
        Self {
            rect,
            uv_rect,
            color,
            kind,
            _padding: [0; 3],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    viewport_size: [f32; 4],
    background_color: [f32; 4],
}

struct Plane {
    texture: wgpu::Texture,
    side: u32,
}

/// Draws every [`GlyphInstance`] in one draw call, sampling whichever of
/// the three atlas planes each instance's `kind` selects.
pub struct TextPipeline {
    pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    monochrome: Plane,
    polychrome: Plane,
    subpixel: Plane,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
}

impl TextPipeline {
    /// `atlas_side` must match the `side` the `GlyphAtlas` this pipeline
    /// will upload from was created with — each plane's `wgpu::Texture` is
    /// sized to match exactly once, at construction.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat, atlas_side: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lux_render::pipeline::text"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lux_render::pipeline::text::layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture_entry(2),
                texture_entry(3),
                texture_entry(4),
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lux_render::pipeline::text::atlas_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let globals_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("lux_render::pipeline::text::globals"),
            contents: bytemuck::bytes_of(&Globals {
                viewport_size: [1.0, 1.0, 0.0, 0.0],
                background_color: [0.0, 0.0, 0.0, 1.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let monochrome = Plane::new(
            device,
            atlas_side,
            wgpu::TextureFormat::R8Unorm,
            "monochrome",
        );
        let polychrome = Plane::new(
            device,
            atlas_side,
            wgpu::TextureFormat::Rgba8Unorm,
            "polychrome",
        );
        let subpixel = Plane::new(
            device,
            atlas_side,
            wgpu::TextureFormat::Rgba8Unorm,
            "subpixel",
        );

        let bind_group = Self::build_bind_group(
            device,
            &bind_group_layout,
            &globals_buffer,
            &sampler,
            &monochrome,
            &polychrome,
            &subpixel,
        );

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lux_render::pipeline::text::pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: size_of::<GlyphInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 32,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 48,
                    shader_location: 3,
                },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lux_render::pipeline::text"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(instance_layout)],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    // Alpha = 1.0 (subpixel's own opaque output) degenerates
                    // this to a plain replace — see this module's doc
                    // comment.
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let instance_capacity = 1024;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lux_render::pipeline::text::instances"),
            size: (instance_capacity * size_of::<GlyphInstance>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            globals_buffer,
            bind_group,
            monochrome,
            polychrome,
            subpixel,
            instance_buffer,
            instance_capacity,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        globals_buffer: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        monochrome: &Plane,
        polychrome: &Plane,
        subpixel: &Plane,
    ) -> wgpu::BindGroup {
        let view = |plane: &Plane| {
            plane
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default())
        };
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lux_render::pipeline::text::bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&view(monochrome)),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&view(polychrome)),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&view(subpixel)),
                },
            ],
        })
    }

    /// Must be called whenever the render target's physical size or the
    /// editor's background color changes, before the next [`Self::draw`].
    /// `background_color` must be the exact color the background quad was
    /// (or will be) painted with — subpixel glyphs composite correctly
    /// only against that specific color, not an arbitrary one.
    pub fn set_globals(
        &self,
        queue: &wgpu::Queue,
        viewport_width: u32,
        viewport_height: u32,
        background_color: [f32; 4],
    ) {
        queue.write_buffer(
            &self.globals_buffer,
            0,
            bytemuck::bytes_of(&Globals {
                viewport_size: [viewport_width as f32, viewport_height as f32, 0.0, 0.0],
                background_color,
            }),
        );
    }

    /// Uploads every byte of all three atlas planes to their GPU textures.
    /// Not incremental — a real optimization for later, once only
    /// re-uploading changed regions matters; correct and real for now.
    pub fn upload_atlas(&self, queue: &wgpu::Queue, atlas: &GlyphAtlas) {
        self.upload_plane(
            queue,
            &self.monochrome,
            atlas.plane_bytes(GlyphKind::Monochrome),
        );
        self.upload_plane(
            queue,
            &self.polychrome,
            atlas.plane_bytes(GlyphKind::Polychrome),
        );
        self.upload_plane(
            queue,
            &self.subpixel,
            atlas.plane_bytes(GlyphKind::Subpixel),
        );
    }

    fn upload_plane(&self, queue: &wgpu::Queue, plane: &Plane, bytes: &[u8]) {
        let bytes_per_pixel = bytes.len() as u32 / (plane.side * plane.side);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &plane.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(plane.side * bytes_per_pixel),
                rows_per_image: Some(plane.side),
            },
            wgpu::Extent3d {
                width: plane.side,
                height: plane.side,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn draw<'pass>(
        &'pass mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'pass>,
        instances: &[GlyphInstance],
    ) {
        if instances.is_empty() {
            return;
        }
        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("lux_render::pipeline::text::instances"),
                size: (self.instance_capacity * size_of::<GlyphInstance>()) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(instances));

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        pass.draw(0..6, 0..instances.len() as u32);
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

impl Plane {
    fn new(device: &wgpu::Device, side: u32, format: wgpu::TextureFormat, label: &str) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("lux_render::pipeline::text::{label}_plane")),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        Self { texture, side }
    }
}
