struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
    _pad: f32,
}

struct NoteInstance {
    @location(0) xywh: vec4<f32>,
    @location(1) rgba: vec4<f32>,
    @location(2) props: vec2<f32>,  // x = corner_radius, y = border_width
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) half_size: vec2<f32>,
    @location(3) radius: f32,
    @location(4) border_width: f32,
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: NoteInstance,
) -> VertexOutput {
    var out: VertexOutput;

    let x = instance.xywh.x;
    let y = instance.xywh.y;
    let w = instance.xywh.z;
    let h = instance.xywh.w;

    var pos = array<vec2<f32>, 6>(
        vec2<f32>(x + w + 1.0, y - 1.0),
        vec2<f32>(x + w + 1.0, y + h + 1.0),
        vec2<f32>(x - 1.0,     y - 1.0),
        vec2<f32>(x + w + 1.0, y + h + 1.0),
        vec2<f32>(x - 1.0,     y + h + 1.0),
        vec2<f32>(x - 1.0,     y - 1.0),
    );

    var uv = array<vec2<f32>, 6>(
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 0.0),
    );

    let pixel_pos = pos[vertex_index];
    let ndc_x = (pixel_pos.x / u.width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pixel_pos.y / u.height) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = instance.rgba;
    out.uv = uv[vertex_index];
    out.half_size = vec2<f32>(w, h) * 0.5;
    out.radius = instance.props.x;
    out.border_width = instance.props.y;
    return out;
}

fn sd_rounded_box(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let d = abs(p) - half + r;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = (in.uv - 0.5) * in.half_size * 2.0;

    // 外层 SDF：整颗音符（含描边），偏移 -0.5px 使边界不透明、消除邻接间隙
    let d_outer = sd_rounded_box(p, in.half_size, in.radius) - 0.5;
    let outer_a = 1.0 - smoothstep(-0.5, 0.5, d_outer);

    // 内层 SDF：填充区域（向内缩进 border_width）
    let inner_half = max(in.half_size - vec2(in.border_width), vec2(0.0));
    let inner_r = max(in.radius - in.border_width, 0.0);
    let d_inner = sd_rounded_box(p, inner_half, inner_r) - 0.5;
    let inner_a = 1.0 - smoothstep(-0.5, 0.5, d_inner);

    let fill_a = inner_a;
    let border_a = outer_a - inner_a;
    let total_a = fill_a + border_a;

    // 描边色 = 填充色 * 0.4（暗色）
    let border_color = in.color.rgb * 0.4;

    // 输出非预乘 alpha 颜色（wgpu ALPHA_BLENDING 使用 SrcAlpha 因子）
    var rgb = vec3(0.0);
    if total_a > 0.0 {
        rgb = (in.color.rgb * fill_a + border_color * border_a) / total_a;
    }
    return vec4(rgb, in.color.a * total_a);
}
