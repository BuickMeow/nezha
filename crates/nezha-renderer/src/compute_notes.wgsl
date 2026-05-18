struct ComputeUniforms {
    time: f32,
    scroll_tick: f32,
    width: f32,
    height: f32,
    speed: f32,
    keyboard_height: f32,
    border_width: f32,
    rounding: f32,
    mode: u32,
    ticks_per_beat: f32,
    equal_key_width: u32,
    key_offset: u32,
    key_count: u32,
}

struct GpuNote {
    start: f32,
    end: f32,
    start_tick: u32,
    end_tick: u32,
    track: u32,
    velocity: u32,
}

struct NoteInstance {
    xywh: vec4<f32>,
    packed: vec4<u32>,  // x=rgba(UNORM8), y=props(2×f16), z=velocity, w=flags
}

struct KeyInfo {
    offset: u32,
    count: u32,
    slot: u32,
}

@group(0) @binding(0) var<uniform> u: ComputeUniforms;
@group(0) @binding(1) var<storage> key_layouts: array<vec2<f32>, 128>;
@group(0) @binding(2) var<storage> key_info: array<KeyInfo, 128>;
@group(0) @binding(3) var<storage> notes: array<GpuNote>;
@group(0) @binding(4) var<storage> palette: array<vec4<f32>, 128>;
@group(0) @binding(5) var<storage, read_write> instances: array<NoteInstance>;
@group(0) @binding(6) var<storage, read_write> instance_count: atomic<u32>;
@group(0) @binding(7) var<storage> key_scans: array<u32, 128>;

const MAX_INSTANCES: u32 = 2700000u;

// ── Packing helpers (match types.rs / shader.wgsl unpack) ────────────────────

fn pack_rgba_vec4(c: vec4<f32>) -> u32 {
    let r8 = u32(clamp(c.r, 0.0, 1.0) * 255.0 + 0.5) & 0xFFu;
    let g8 = u32(clamp(c.g, 0.0, 1.0) * 255.0 + 0.5) & 0xFFu;
    let b8 = u32(clamp(c.b, 0.0, 1.0) * 255.0 + 0.5) & 0xFFu;
    let a8 = u32(clamp(c.a, 0.0, 1.0) * 255.0 + 0.5) & 0xFFu;
    return r8 | (g8 << 8u) | (b8 << 16u) | (a8 << 24u);
}

fn pack_props(radius: f32, border: f32) -> u32 {
    return pack2x16float(vec2<f32>(radius, border));
}

fn write_instance(idx: u32, x: f32, y: f32, w: f32, h: f32,
                  trk: u32, radius: f32, border: f32, vel: u32) {
    if idx < MAX_INSTANCES {
        instances[idx] = NoteInstance(
            vec4<f32>(x, y, w, h),
            vec4<u32>(
                pack_rgba_vec4(vec4<f32>(palette[trk].rgb, 1.0)),
                pack_props(radius, border),
                vel,
                0u,  // flags reserved
            ),
        );
    }
}

/// Shared emission logic: compute border/rounding and atomically append.
fn emit_note(x: f32, y: f32, w: f32, h: f32, track: u32, velocity: u32) {
    let border_px = u.border_width * w / 2.0;
    let rounding_radius = u.rounding * min(w, h);
    let idx = atomicAdd(&instance_count, 1u);
    write_instance(idx, x, y, w, h, track, rounding_radius, border_px, velocity);
}

// ── Main ──────────────────────────────────────────────────────────────────────

@compute
@workgroup_size(64)
fn compute_notes(
    @builtin(workgroup_id) wg_id: vec3<u32>,
    @builtin(local_invocation_index) lid: u32,
) {
    let k = wg_id.x * 64u + lid;
    if k >= u.key_count {
        return;
    }
    let key = u.key_offset + k;

    let info = key_info[key];
    let count = info.count;
    if count == 0u {
        return;
    }

    let offset = info.offset;
    let kl = key_layouts[key];
    let x = kl.x;
    let w = kl.y;
    if w <= 0.0 {
        return;
    }

    let kh = max(u.keyboard_height, 0.0);
    let effective_h = max(u.height - kh, 1.0);
    let scan_start = key_scans[key];

    if u.mode == 0u {
        let pps = 200.0 * max(u.speed, 0.01);
        let screen_top = effective_h + u.time * pps;
        let time_top = u.time + effective_h / pps + 1.0;
        let time_bottom = u.time;

        for (var i = scan_start; i < count; i++) {
            let note = notes[offset + i];

            if note.start > time_top { break; }
            if note.end <= time_bottom { continue; }

            let note_bottom = screen_top - note.start * pps;
            let note_top_val = screen_top - note.end * pps;
            let y = note_top_val;
            let h = max(note_bottom - note_top_val, 1.0);

            emit_note(x, y, w, h, note.track % 128u, note.velocity);
        }
    } else {
        let ppt = 100.0 / u.ticks_per_beat * max(u.speed, 0.01);
        let visible_ticks = effective_h / ppt;
        let tick_at_bottom = u.scroll_tick;
        let tick_at_top = u.scroll_tick + visible_ticks;
        let screen_bottom = effective_h + u.scroll_tick * ppt;

        for (var i = scan_start; i < count; i++) {
            let note = notes[offset + i];

            let start_tick = f32(note.start_tick);
            let end_tick = f32(note.end_tick);
            if start_tick > tick_at_top + 1.0 { break; }
            if end_tick <= tick_at_bottom { continue; }

            let note_top_val = screen_bottom - end_tick * ppt;
            let note_bottom = screen_bottom - start_tick * ppt;
            let y = note_top_val;
            let h = max(note_bottom - note_top_val, 1.0);

            emit_note(x, y, w, h, note.track % 128u, note.velocity);
        }
    }
}
