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
}

struct NoteInstance {
    xywh: vec4<f32>,
    rgba: vec4<f32>,
    props: vec2<f32>,
    _pad: vec2<f32>,
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
@group(0) @binding(8) var<storage, read_write> keyboard_instances: array<NoteInstance, 128>;

const MAX_INSTANCES: u32 = 2700000u;

fn is_black_key(k: u32) -> bool {
    let m = k % 12u;
    return m == 1u || m == 3u || m == 6u || m == 8u || m == 10u;
}

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

    var key_active: bool = false;
    var active_trk: u32 = 0u;

    if u.mode == 0u {
        let pps = 200.0 * max(u.speed, 0.01);
        let screen_top = effective_h + u.time * pps;
        let visible_future = effective_h / pps + 1.0;
        let time_top = u.time + visible_future;
        let time_bottom = u.time;

        for (var i = scan_start; i < count; i++) {
            let note = notes[offset + i];

            if note.start > time_top { break; }
            if note.end <= time_bottom { continue; }

            if note.start <= u.time && u.time < note.end {
                key_active = true;
                active_trk = note.track % 128u;
            }

            let note_bottom = screen_top - note.start * pps;
            let note_top_val = screen_top - note.end * pps;
            let y = note_top_val;
            let h = max(note_bottom - note_top_val, 1.0);

            let trk = note.track % 128u;
            let border_px = u.border_width * w / 2.0;
            let rounding_radius = u.rounding * min(w, h);

            let idx = atomicAdd(&instance_count, 1u);
            if idx < MAX_INSTANCES {
                instances[idx] = NoteInstance(
                    vec4<f32>(x, y, w, h),
                    vec4<f32>(palette[trk].r, palette[trk].g, palette[trk].b, 1.0),
                    vec2<f32>(rounding_radius, border_px),
                    vec2<f32>(0.0, 0.0),
                );
            }
        }
    } else {
        let eff_speed = max(u.speed, 0.01);
        let ppt = 100.0 / u.ticks_per_beat * eff_speed;
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

            if note.start <= u.time && u.time < note.end {
                key_active = true;
                active_trk = note.track % 128u;
            }

            let note_top_val = screen_bottom - end_tick * ppt;
            let note_bottom = screen_bottom - start_tick * ppt;
            let y = note_top_val;
            let h = max(note_bottom - note_top_val, 1.0);

            let trk = note.track % 128u;
            let border_px = u.border_width * w / 2.0;
            let rounding_radius = u.rounding * min(w, h);

            let idx = atomicAdd(&instance_count, 1u);
            if idx < MAX_INSTANCES {
                instances[idx] = NoteInstance(
                    vec4<f32>(x, y, w, h),
                    vec4<f32>(palette[trk].r, palette[trk].g, palette[trk].b, 1.0),
                    vec2<f32>(rounding_radius, border_px),
                    vec2<f32>(0.0, 0.0),
                );
            }
        }
    }

    // Write keyboard instance for this key
    let slot = key_info[key].slot;
    let key_top = u.height - kh;
    if is_black_key(key) {
        let c = select(vec3<f32>(0.16, 0.16, 0.17), palette[active_trk].rgb, key_active);
        keyboard_instances[slot] = NoteInstance(
            vec4<f32>(x, key_top, w, kh * 0.6),
            vec4<f32>(c, 1.0),
            vec2<f32>(1.5, 0.5),
            vec2<f32>(0.0, 0.0),
        );
    } else {
        let c = select(vec3<f32>(0.94, 0.94, 0.94), palette[active_trk].rgb, key_active);
        keyboard_instances[slot] = NoteInstance(
            vec4<f32>(x, key_top, w, kh),
            vec4<f32>(c, 1.0),
            vec2<f32>(2.0, 0.5),
            vec2<f32>(0.0, 0.0),
        );
    }
}
