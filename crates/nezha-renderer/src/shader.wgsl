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

    // TriangleList: 6 个顶点组成两个三角形
    // 0,1,2: (x+w,y) → (x+w,y+h) → (x,y)     // 右上→右下→左上
    // 3,4,5: (x+w,y+h) → (x,y+h) → (x,y)     // 右下→左下→左上
    var pos = array<vec2<f32>, 6>(
        vec2<f32>(x + w, y),     // 0
        vec2<f32>(x + w, y + h), // 1
        vec2<f32>(x,     y),     // 2
        vec2<f32>(x + w, y + h), // 3
        vec2<f32>(x,     y + h), // 4
        vec2<f32>(x,     y),     // 5
    );

    let pixel_pos = pos[vertex_index];
    let ndc_x = (pixel_pos.x / u.width) * 2.0 - 1.0;
    let ndc_y = 1.0 - (pixel_pos.y / u.height) * 2.0;

    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = instance.rgba;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
