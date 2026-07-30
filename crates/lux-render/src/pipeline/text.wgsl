struct Globals {
    // .xy = viewport size in physical pixels, .zw unused padding.
    viewport_size: vec4<f32>,
    // The solid color already painted beneath this text (the editor
    // background quad). Subpixel compositing (`kind == 2u`) is only
    // correct against a known, opaque background — see
    // `lux_render::pipeline::text`'s own doc comment for why.
    background_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var atlas_sampler: sampler;
@group(0) @binding(2) var monochrome_tex: texture_2d<f32>;
@group(0) @binding(3) var polychrome_tex: texture_2d<f32>;
@group(0) @binding(4) var subpixel_tex: texture_2d<f32>;

struct Instance {
    // x, y, width, height, in physical pixels, top-left origin, y-down.
    @location(0) rect: vec4<f32>,
    // u0, v0, u1, v1 — normalized (0..1) atlas texture coordinates.
    @location(1) uv_rect: vec4<f32>,
    // Tint color for Monochrome/Subpixel; ignored for Polychrome (already
    // has its own real color, e.g. emoji).
    @location(2) color: vec4<f32>,
    // 0 = Monochrome, 1 = Polychrome, 2 = Subpixel — matches
    // `lux_render::glyph_atlas::GlyphKind`'s three variants exactly.
    @location(3) kind: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) kind: u32,
};

const CORNERS: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
);

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, instance: Instance) -> VertexOutput {
    let corner = CORNERS[vertex_index];
    let pixel = instance.rect.xy + corner * instance.rect.zw;
    let ndc_x = (pixel.x / globals.viewport_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pixel.y / globals.viewport_size.y) * 2.0;

    var out: VertexOutput;
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = instance.uv_rect.xy + corner * (instance.uv_rect.zw - instance.uv_rect.xy);
    out.color = instance.color;
    out.kind = instance.kind;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.kind == 0u) {
        // Monochrome: single-channel coverage, tinted by `color`. Regular
        // alpha blending composites this correctly against anything
        // already drawn, transparent or not.
        let coverage = textureSample(monochrome_tex, atlas_sampler, in.uv).r;
        return vec4<f32>(in.color.rgb, in.color.a * coverage);
    } else if (in.kind == 1u) {
        // Polychrome: already-colored bitmap (emoji, COLR layers) — no
        // tint, sample and composite as-is.
        return textureSample(polychrome_tex, atlas_sampler, in.uv);
    } else {
        // Subpixel: the mask's own R/G/B channels are per-channel coverage
        // (LCD subpixel AA), which can only composite correctly against a
        // known, solid background — there is no dual-source-blending
        // trick used here, just the classic ClearType-era technique of
        // mixing foreground and background per channel entirely in the
        // shader, then outputting fully opaque. Regular alpha blending
        // with alpha = 1.0 degenerates to a plain replace, so this still
        // composites correctly through the same blend state the other two
        // kinds use.
        let mask = textureSample(subpixel_tex, atlas_sampler, in.uv).rgb;
        let rgb = mix(globals.background_color.rgb, in.color.rgb, mask);
        return vec4<f32>(rgb, 1.0);
    }
}
