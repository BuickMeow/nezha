struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
}

@group(0) @binding(0)
var<uniform> u: Uniforms;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4<f32> {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    return vec4<f32>(pos[in_vertex_index], 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag_coord.xy / vec2<f32>(u.width, u.height);
    let color = vec3<f32>(
        0.5 + 0.5 * cos(u.time + uv.x * 3.14159 + 0.0),
        0.5 + 0.5 * cos(u.time + uv.y * 3.14159 + 2.094),
        0.5 + 0.5 * cos(u.time + (uv.x + uv.y) * 3.14159 + 4.188),
    );
    return vec4<f32>(color, 1.0);
}
