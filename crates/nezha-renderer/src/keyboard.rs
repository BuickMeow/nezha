use crate::state::MidiRenderState;
use crate::style::NoteSource;
use crate::types::{NoteInstance, pack_props, pack_rgba};

// ── Keyboard appearance constants ───────────────────────────────────────────

/// Default white key color (light grey).
const WHITE_KEY_COLOR: (f32, f32, f32) = (0.94, 0.94, 0.94);
/// Default black key color (dark grey).
const BLACK_KEY_COLOR: (f32, f32, f32) = (0.16, 0.16, 0.17);
/// Black key height as a fraction of total keyboard height.
const BLACK_KEY_HEIGHT_RATIO: f32 = 0.6;
/// Corner radius for white keys.
const WHITE_KEY_CORNER_RADIUS: f32 = 2.0;
/// Corner radius for black keys.
const BLACK_KEY_CORNER_RADIUS: f32 = 1.5;
/// Border width for all keys.
const KEY_BORDER_WIDTH: f32 = 0.5;

pub(crate) fn is_black_key(key: u8) -> bool {
    matches!(key % 12, 1 | 3 | 6 | 8 | 10)
}

pub(crate) fn compute_key_layouts(width: u32, equal_width: bool) -> Vec<(f32, f32)> {
    let mut layouts = Vec::with_capacity(128);
    if equal_width {
        let key_w = width as f64 / 128.0;
        for key in 0..128 {
            let x = (key as f64 * key_w).round() as f32;
            let next_x = ((key as f64 + 1.0) * key_w).round() as f32;
            let w = (next_x - x).max(1.0);
            layouts.push((x, w));
        }
    } else {
        let white_width = width as f64 / 75.0;
        let black_width = white_width * 0.65;
        let mut white_count = 0usize;
        for key in 0..128u8 {
            if is_black_key(key) {
                let x = (white_count as f64 * white_width - black_width * 0.5).round() as f32;
                let w = black_width.round() as f32;
                layouts.push((x, w.max(1.0)));
            } else {
                let x = (white_count as f64 * white_width).round() as f32;
                let next_x = ((white_count + 1) as f64 * white_width).round() as f32;
                let w = (next_x - x).max(1.0);
                layouts.push((x, w));
                white_count += 1;
            }
        }
    }
    layouts
}

pub(crate) fn build_keyboard_instances(
    width: u32,
    height: u32,
    time: f64,
    midi: &dyn NoteSource,
    keyboard_height: f32,
    equal_key_width: bool,
    palette: &[[f32; 3]; 128],
    state: &MidiRenderState,
) -> Vec<NoteInstance> {
    let kh = keyboard_height.max(1.0);
    let key_top = height as f32 - kh;
    let layouts = compute_key_layouts(width, equal_key_width);

    let mut active_keys = [false; 128];
    let mut active_colors = [[0.0f32; 3]; 128];
    for key in 0..128u8 {
        let notes = midi.key_notes(key);
        let scan = state.scan_indices[key as usize];
        for note in notes[scan..].iter() {
            if note.start > time {
                break;
            }
            if time < note.end {
                active_keys[key as usize] = true;
                let trk = note.track as usize % 128;
                active_colors[key as usize] = palette[trk];
                break;
            }
        }
    }

    let mut instances = Vec::with_capacity(256);
    let black_h = kh * BLACK_KEY_HEIGHT_RATIO;

    // White keys first
    for key in 0..128u8 {
        if is_black_key(key) {
            continue;
        }
        let (x, w) = layouts[key as usize];
        if w <= 0.0 {
            continue;
        }
        let (r, g, b) = if active_keys[key as usize] {
            let [cr, cg, cb] = active_colors[key as usize];
            (cr, cg, cb)
        } else {
            WHITE_KEY_COLOR
        };
        instances.push(NoteInstance {
            x,
            y: key_top,
            w,
            h: kh,
            rgba_packed: pack_rgba(r, g, b, 1.0),
            props_packed: pack_props(WHITE_KEY_CORNER_RADIUS, KEY_BORDER_WIDTH),
            velocity: 0,
            flags: 0,
        });
    }

    // Black keys on top
    for key in 0..128u8 {
        if !is_black_key(key) {
            continue;
        }
        let (x, w) = layouts[key as usize];
        if w <= 0.0 {
            continue;
        }
        let (r, g, b) = if active_keys[key as usize] {
            let [cr, cg, cb] = active_colors[key as usize];
            (cr, cg, cb)
        } else {
            BLACK_KEY_COLOR
        };
        instances.push(NoteInstance {
            x,
            y: key_top,
            w,
            h: black_h,
            rgba_packed: pack_rgba(r, g, b, 1.0),
            props_packed: pack_props(BLACK_KEY_CORNER_RADIUS, KEY_BORDER_WIDTH),
            velocity: 0,
            flags: 0,
        });
    }

    instances
}
