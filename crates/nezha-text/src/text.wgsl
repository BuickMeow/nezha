struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct Uniforms {
    color: vec4<f32>,
    screen_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var atlas_texture: texture_2d<f32>;

@group(0) @binding(2)
var atlas_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let p = in.position;
    let clip_x = p.x / uniforms.screen_size.x * 2.0 - 1.0;
    let clip_y = 1.0 - p.y / uniforms.screen_size.y * 2.0;
    out.position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = textureSample(atlas_texture, atlas_sampler, in.uv).r;
    return vec4<f32>(uniforms.color.rgb, uniforms.color.a * alpha);
}
