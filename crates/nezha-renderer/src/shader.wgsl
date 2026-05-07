struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
    _pad: f32,
}

struct NoteInstance {
    @location(0) xywh: vec4<f32>,
    @location(1) rgba: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) rect_min: vec2<f32>,
    @location(2) rect_max: vec2<f32>,
}

struct FragmentInput {
    @location(0) color: vec4<f32>,
    @location(1) rect_min: vec2<f32>,
    @location(2) rect_max: vec2<f32>,
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
        vec2<f32>(x + w, y),
        vec2<f32>(x + w, y + h),
        vec2<f32>(x,     y),
        vec2<f32>(x + w, y + h),
        vec2<f32>(x,     y + h),
        vec2<f32>(x,     y),
    );

    let pixel_pos = pos[vertex_index];
    let ndc_x = (pixel_pos.x / u.width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pixel_pos.y / u.height) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = instance.rgba;
    out.rect_min = vec2<f32>(x, y);
    out.rect_max = vec2<f32>(x + w, y + h);
    return out;
}

@fragment
fn fs_main(
    @builtin(position) frag_coord: vec4<f32>,
    in: FragmentInput,
) -> @location(0) vec4<f32> {
    let dx = min(frag_coord.x - in.rect_min.x, in.rect_max.x - frag_coord.x);
    let dy = min(frag_coord.y - in.rect_min.y, in.rect_max.y - frag_coord.y);
    let d = min(dx, dy);

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
