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
    @location(2) rect_size: vec2<f32>,
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
    out.rect_size = vec2<f32>(w, h);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // 到矩形四边的像素距离
    let px_w = in.rect_size.x;
    let px_h = in.rect_size.y;
    let dx = min(uv.x, 1.0 - uv.x) * px_w;
    let dy = min(uv.y, 1.0 - uv.y) * px_h;
    let d = min(dx, dy);

    // 固定 1.5px 描边，硬切边（无渐变）
    let border_px = 1.5;
    let is_border = d < border_px;

    let border_color = in.color * vec4<f32>(0.4, 0.4, 0.4, 1.0);
    let fill_color = in.color;

    if is_border {
        return border_color;
    } else {
        return fill_color;
    }
}
