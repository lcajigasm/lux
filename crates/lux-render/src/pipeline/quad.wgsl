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
    // `viewport_size.z > 0` means the render target is sRGB. Authored
    // colors are sRGB bytes/255; an sRGB target stores linear and encodes
    // on write, so passing the bytes through would lighten every dark
    // Monokai well (the #272822 editor became about #6D6E66).
    out.color = vec4<f32>(authored_rgb(instance.color.rgb), instance.color.a);
    return out;
}

fn srgb_channel_to_linear(c: f32) -> f32 {
    if (c <= 0.04045) {
        return c / 12.92;
    }
    return pow((c + 0.055) / 1.055, 2.4);
}

fn authored_rgb(rgb: vec3<f32>) -> vec3<f32> {
    if (globals.viewport_size.z < 0.5) {
        return rgb;
    }
    return vec3<f32>(
        srgb_channel_to_linear(rgb.r),
        srgb_channel_to_linear(rgb.g),
        srgb_channel_to_linear(rgb.b),
    );
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
