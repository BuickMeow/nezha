struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
}

struct NoteInstance {
    @location(0) xywh: vec4<f32>,
    @location(1) rgba: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
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
        vec2<f32>(x + w, y),     // 0: 右上
        vec2<f32>(x + w, y + h), // 1: 右下
        vec2<f32>(x,     y),     // 2: 左上
        vec2<f32>(x + w, y + h), // 3: 右下
        vec2<f32>(x,     y + h), // 4: 左下
        vec2<f32>(x,     y),     // 5: 左上
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
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // 到矩形四边的最小距离（UV 空间 0~1）
    let d = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));

    // 固定 UV 宽度的描边，不依赖 fwidth（避免 instance rendering 闪烁）
    // 0.015 ≈ 1~2px 在常见分辨率下
    let border_uv = 0.015;

    // 描边：外侧 darker，内侧 fill
    let border_color = in.color * vec4<f32>(0.4, 0.4, 0.4, 1.0);
    let fill_color = in.color;

    // smoothstep 自带抗锯齿，阈值固定不闪烁
    let t = smoothstep(0.0, border_uv, d);
    let final_color = mix(border_color, fill_color, t);

    return final_color;
}
