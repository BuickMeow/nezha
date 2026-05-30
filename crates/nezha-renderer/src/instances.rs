use rayon::prelude::*;

use crate::constants::{MIN_SPEED, PIXELS_PER_SEC_BASE};
use crate::key_order::{build_parallel_key_groups, build_render_key_order};
use crate::keyboard;
use crate::scan::{NoteSeekIndex, advance_scan_indices, scroll_tick_for_mode};
use crate::source::NoteSource;
use crate::state::MidiRenderState;
use crate::style::{RenderMode, RenderStyle};
use crate::vertex::{NoteInstance, pack_props, pack_rgba};

/// Temporary result during parallel key chunk processing.
pub(crate) struct KeyChunkBuildResult {
    pub(crate) instances: Vec<NoteInstance>,
    pub(crate) active_keys: [bool; 128],
    pub(crate) active_colors: [[f32; 3]; 128],
}

impl KeyChunkBuildResult {
    pub(crate) fn new() -> Self {
        Self {
            instances: Vec::new(),
            active_keys: [false; 128],
            active_colors: [[0.0; 3]; 128],
        }
    }
}

/// Build note instances for the current frame.
///
/// Returns the number of note instances (excluding keyboard overlay instances).
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_instances(
    instances: &mut Vec<NoteInstance>,
    layouts: &[(f32, f32)],
    width: u32,
    height: u32,
    time: f64,
    speed: f32,
    midi: &dyn NoteSource,
    state: &mut MidiRenderState,
    seek_index: Option<&NoteSeekIndex>,
    style: &RenderStyle,
) -> usize {
    let mut active_keys = [false; 128];
    let mut active_colors = [[0.0f32; 3]; 128];

    let scroll_tick = scroll_tick_for_mode(midi, time, style);
    advance_scan_indices(
        midi,
        state,
        time,
        scroll_tick,
        style.render_mode,
        seek_index,
    );
    let scan_indices = state.scan_indices;
    let render_keys = build_render_key_order(style.equal_key_width);

    match style.render_mode {
        RenderMode::TimeBased => build_instances_time(
            instances,
            layouts,
            &render_keys,
            &scan_indices,
            &mut active_keys,
            &mut active_colors,
            height,
            time,
            speed,
            midi,
            style,
        ),
        RenderMode::TickBased => build_instances_tick(
            instances,
            layouts,
            &render_keys,
            &scan_indices,
            &mut active_keys,
            &mut active_colors,
            height,
            time,
            speed,
            midi,
            style,
        ),
    };

    let note_count = instances.len();
    if style.keyboard_height > 0.0 {
        keyboard::append_keyboard_instances(
            layouts,
            width,
            height,
            style.keyboard_height,
            style.equal_key_width,
            &active_keys,
            &active_colors,
            instances,
        );
    }
    note_count
}

/// Build a single `NoteInstance` from screen-space coordinates and color.
#[allow(clippy::too_many_arguments)]
fn build_note_instance(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    g: f32,
    b: f32,
    velocity: u8,
    style: &RenderStyle,
) -> NoteInstance {
    NoteInstance {
        x,
        y,
        w,
        h,
        rgba_packed: pack_rgba(r, g, b, 1.0),
        props_packed: pack_props(
            style.rounding * f32::min(w, h),
            style.border_width * w / 2.0,
        ),
        velocity: velocity as u32,
        flags: 0,
    }
}

/// Merge parallel chunk results into the shared instance buffer and active-key arrays.
fn merge_chunk_results(
    chunk_results: Vec<KeyChunkBuildResult>,
    instances: &mut Vec<NoteInstance>,
    active_keys: &mut [bool; 128],
    active_colors: &mut [[f32; 3]; 128],
) {
    for chunk in chunk_results {
        for key in 0..128usize {
            if chunk.active_keys[key] {
                active_keys[key] = true;
                active_colors[key] = chunk.active_colors[key];
            }
        }
        instances.extend(chunk.instances);
    }
}

/// Run the parallel key-group orchestration, calling `per_key_fn` for each key.
///
/// This is the shared scaffolding used by both time-based and tick-based builders.
fn build_instances_parallel(
    render_keys: &[u8; 128],
    scan_indices: &[usize; 128],
    midi: &dyn NoteSource,
    instances: &mut Vec<NoteInstance>,
    active_keys: &mut [bool; 128],
    active_colors: &mut [[f32; 3]; 128],
    per_key_fn: impl Fn(&mut KeyChunkBuildResult, u8, usize) + Sync,
) {
    let key_groups = build_parallel_key_groups(render_keys, scan_indices, midi);
    let chunk_results = key_groups
        .into_par_iter()
        .map(|range| {
            let mut result = KeyChunkBuildResult::new();
            for &key in &render_keys[range] {
                per_key_fn(&mut result, key, scan_indices[key as usize]);
            }
            result
        })
        .collect::<Vec<_>>();

    merge_chunk_results(chunk_results, instances, active_keys, active_colors);
}

fn build_instances_time(
    instances: &mut Vec<NoteInstance>,
    layouts: &[(f32, f32)],
    render_keys: &[u8; 128],
    scan_indices: &[usize; 128],
    active_keys: &mut [bool; 128],
    active_colors: &mut [[f32; 3]; 128],
    height: u32,
    time: f64,
    speed: f32,
    midi: &dyn NoteSource,
    style: &RenderStyle,
) {
    let kh = (style.keyboard_height as f64).max(0.0);
    let effective_h = (height as f64 - kh).max(1.0);
    let pps = PIXELS_PER_SEC_BASE * speed.max(MIN_SPEED) as f64;
    let screen_top = effective_h + time * pps;
    let time_top = time + effective_h / pps;
    let time_bottom = time;

    build_instances_parallel(
        render_keys,
        scan_indices,
        midi,
        instances,
        active_keys,
        active_colors,
        |result, key, scan| {
            append_key_instances_time(
                result, key, layouts, scan, time, time_top, time_bottom, screen_top, pps, midi,
                style,
            );
        },
    );
}

fn build_instances_tick(
    instances: &mut Vec<NoteInstance>,
    layouts: &[(f32, f32)],
    render_keys: &[u8; 128],
    scan_indices: &[usize; 128],
    active_keys: &mut [bool; 128],
    active_colors: &mut [[f32; 3]; 128],
    height: u32,
    time: f64,
    speed: f32,
    midi: &dyn NoteSource,
    style: &RenderStyle,
) {
    let kh = (style.keyboard_height as f64).max(0.0);
    let effective_h = (height as f64 - kh).max(1.0);
    let ticks_per_beat = midi.ticks_per_beat().unwrap_or(480) as f64;
    let ppt = 100.0 / ticks_per_beat * speed.max(MIN_SPEED) as f64;
    let scroll_tick = midi
        .tick_at_time(time)
        .unwrap_or(time * ticks_per_beat * 2.0);
    let visible_ticks = effective_h / ppt;
    let tick_at_top = scroll_tick + visible_ticks;
    let screen_bottom = effective_h + scroll_tick * ppt;

    build_instances_parallel(
        render_keys,
        scan_indices,
        midi,
        instances,
        active_keys,
        active_colors,
        |result, key, scan| {
            append_key_instances_tick(
                result, key, layouts, scan, time, tick_at_top, scroll_tick, screen_bottom, ppt,
                midi, style,
            );
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn append_key_instances_time(
    result: &mut KeyChunkBuildResult,
    key: u8,
    layouts: &[(f32, f32)],
    scan: usize,
    time: f64,
    time_top: f64,
    time_bottom: f64,
    screen_top: f64,
    pps: f64,
    midi: &dyn NoteSource,
    style: &RenderStyle,
) {
    let notes = midi.key_notes(key);
    if notes.is_empty() {
        return;
    }
    let (x, w) = layouts[key as usize];

    for note in &notes[scan.min(notes.len())..] {
        if note.start > time_top {
            break;
        }
        if note.end <= time_bottom {
            continue;
        }

        let trk = note.track as usize % 128;
        let [r, g, b] = style.palette[trk];
        if note.start <= time && time < note.end {
            result.active_keys[key as usize] = true;
            result.active_colors[key as usize] = [r, g, b];
        }

        let note_bottom = (screen_top - note.start * pps) as f32;
        let note_top = (screen_top - note.end * pps) as f32;
        let h = (note_bottom - note_top).max(1.0);
        result
            .instances
            .push(build_note_instance(x, note_top, w, h, r, g, b, note.velocity, style));
    }
}

#[allow(clippy::too_many_arguments)]
fn append_key_instances_tick(
    result: &mut KeyChunkBuildResult,
    key: u8,
    layouts: &[(f32, f32)],
    scan: usize,
    time: f64,
    tick_at_top: f64,
    scroll_tick: f64,
    screen_bottom: f64,
    ppt: f64,
    midi: &dyn NoteSource,
    style: &RenderStyle,
) {
    let notes = midi.key_notes(key);
    if notes.is_empty() {
        return;
    }
    let (x, w) = layouts[key as usize];

    for note in &notes[scan.min(notes.len())..] {
        if (note.start_tick as f64) > tick_at_top + 1.0 {
            break;
        }
        if (note.end_tick as f64) <= scroll_tick {
            continue;
        }

        let trk = note.track as usize % 128;
        let [r, g, b] = style.palette[trk];
        if note.start <= time && time < note.end {
            result.active_keys[key as usize] = true;
            result.active_colors[key as usize] = [r, g, b];
        }

        let note_top = (screen_bottom - note.end_tick as f64 * ppt) as f32;
        let note_bottom = (screen_bottom - note.start_tick as f64 * ppt) as f32;
        let h = (note_bottom - note_top).max(1.0);
        result
            .instances
            .push(build_note_instance(x, note_top, w, h, r, g, b, note.velocity, style));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_chunk_build_result_new() {
        let result = KeyChunkBuildResult::new();
        assert!(result.instances.is_empty());
        assert_eq!(result.active_keys, [false; 128]);
        for c in &result.active_colors {
            assert_eq!(c, &[0.0; 3]);
        }
    }

    #[test]
    fn test_key_chunk_build_result_accumulate() {
        let mut result = KeyChunkBuildResult::new();

        result.active_keys[60] = true;
        result.active_colors[60] = [1.0, 0.0, 0.0];
        result.instances.push(NoteInstance {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            rgba_packed: 0xFFFFFFFF,
            props_packed: 0,
            velocity: 100,
            flags: 0,
        });

        assert_eq!(result.instances.len(), 1);
        assert!(result.active_keys[60]);
        assert!(!result.active_keys[0]);
        assert_eq!(result.active_colors[60], [1.0, 0.0, 0.0]);
    }
}
