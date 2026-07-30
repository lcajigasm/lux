struct Globals {
    // .xy = viewport size in physical pixels, .zw unused padding.
    viewport_size: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

struct Instance {
    // x, y, width, height, in physical pixels, top-left origin, y-down —
    // the same coordinate space every other Lux geometry type already uses
    // (`lux_platform::PhysicalPosition`, `lux_ui::editor_view`'s rects).
    @location(0) rect: vec4<f32>,
    // Straight (non-premultiplied) sRGB-space RGBA, 0..1.
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// A unit quad's corners, indexed by `vertex_index` — two triangles, no
// vertex buffer needed for geometry, only the per-instance rect/color.
const CORNERS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
);

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, instance: Instance) -> VertexOutput {
    let corner = CORNERS[vertex_index];
    let pixel = instance.rect.xy + corner * instance.rect.zw;
    let ndc_x = (pixel.x / globals.viewport_size.x) * 2.0 - 1.0;
    // wgpu's NDC is y-up; our rects are y-down, so flip.
    let ndc_y = 1.0 - (pixel.y / globals.viewport_size.y) * 2.0;

    var out: VertexOutput;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = instance.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
