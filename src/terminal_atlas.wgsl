struct ViewportUniform {
    size: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> viewport: ViewportUniform;

@group(1) @binding(0)
var glyph_atlas: texture_2d_array<f32>;

@group(1) @binding(1)
var glyph_sampler: sampler;

struct RectVertexIn {
    @location(0) rect: vec4<f32>,
    @location(1) color: vec4<f32>,
};

struct RectVertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

struct GlyphVertexIn {
    @location(0) rect: vec4<f32>,
    @location(1) uv_rect: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) extras: vec4<f32>,
};

struct GlyphVertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) layer: f32,
};

fn quad_corner(index: u32) -> vec2<f32> {
    switch index {
        case 0u: { return vec2<f32>(0.0, 0.0); }
        case 1u: { return vec2<f32>(0.0, 1.0); }
        case 2u: { return vec2<f32>(1.0, 0.0); }
        default: { return vec2<f32>(1.0, 1.0); }
    }
}

fn to_clip_space(pixel: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        (pixel.x / viewport.size.x) * 2.0 - 1.0,
        1.0 - (pixel.y / viewport.size.y) * 2.0,
    );
}

@vertex
fn rect_vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: RectVertexIn,
) -> RectVertexOut {
    let corner = quad_corner(vertex_index);
    let pixel = instance.rect.xy + corner * instance.rect.zw;

    var out: RectVertexOut;
    out.position = vec4<f32>(to_clip_space(pixel), 0.0, 1.0);
    out.color = instance.color;
    return out;
}

@fragment
fn rect_fs_main(in: RectVertexOut) -> @location(0) vec4<f32> {
    return in.color;
}

@vertex
fn glyph_vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: GlyphVertexIn,
) -> GlyphVertexOut {
    let corner = quad_corner(vertex_index);
    let pixel = instance.rect.xy + corner * instance.rect.zw;

    var out: GlyphVertexOut;
    out.position = vec4<f32>(to_clip_space(pixel), 0.0, 1.0);
    out.uv = instance.uv_rect.xy + corner * (instance.uv_rect.zw - instance.uv_rect.xy);
    out.color = instance.color;
    out.layer = instance.extras.x;
    return out;
}

@fragment
fn glyph_fs_main(in: GlyphVertexOut) -> @location(0) vec4<f32> {
    let coverage = textureSample(glyph_atlas, glyph_sampler, in.uv, i32(in.layer + 0.5)).r;
    return vec4<f32>(in.color.rgb, in.color.a * coverage);
}
