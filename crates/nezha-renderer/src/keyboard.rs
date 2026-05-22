use crate::vertex::{NoteInstance, pack_props, pack_rgba};
pub(crate) use nezha_core::is_black_key;

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

/// Compute screen-space x-offset and width for each of the 128 keys.
///
/// When `equal_width` is true, all 128 keys get an equal share of the total width.
/// When `equal_width` is false, the 75 white keys evenly divide the total width
/// and black keys are placed at the correct boundaries between white keys,
/// overlaid on top with a narrower width (65% of the white-key width).
pub(crate) fn compute_key_layouts(width: u32, equal_width: bool) -> Vec<(f32, f32)> {
    let mut layouts = Vec::with_capacity(128);
    if equal_width {
        let key_w = width as f64 / 128.0;
        for key in 0..128 {
            let x = (key as f64 * key_w) as f32;
            layouts.push((x, key_w as f32));
        }
    } else {
        // Piano layout: all white keys evenly divide the total width.
        // Black keys are centred at the boundary between adjacent white keys
        // and overlaid on top with a narrower width.
        let total_w = width as f64;
        let white_key_count = (0..128u8).filter(|&k| !is_black_key(k)).count() as f64;
        let white_w = total_w / white_key_count;
        let black_w = white_w * 0.65;

        let mut white_count = 0usize;
        for key in 0..128u8 {
            if is_black_key(key) {
                // Black key centred at the boundary between
                // white key (white_count-1) and white key (white_count).
                let boundary_x = white_count as f64 * white_w;
                let x = (boundary_x - black_w * 0.5) as f32;
                layouts.push((x, black_w as f32));
            } else {
                let x = (white_count as f64 * white_w) as f32;
                layouts.push((x, white_w as f32));
                white_count += 1;
            }
        }
    }
    layouts
}

/// Build vertex instances for the on-screen piano keyboard.
///
/// `active_keys` / `active_colors` should reflect the top-most currently playing
/// note per key, already resolved by the main waterfall scan.
///
/// When `equal_key_width` is true, the white keys are expanded so that they fill
/// the entire keyboard bottom without gaps, while black keys keep their original
/// equal-width positions and are drawn on top.
pub(crate) fn append_keyboard_instances(
    layouts: &[(f32, f32)],
    width: u32,
    height: u32,
    keyboard_height: f32,
    equal_key_width: bool,
    active_keys: &[bool; 128],
    active_colors: &[[f32; 3]; 128],
    out: &mut Vec<NoteInstance>,
) {
    let kh = keyboard_height.max(1.0);
    let key_top = height as f32 - kh;
    let black_h = kh * BLACK_KEY_HEIGHT_RATIO;
    out.reserve(128);

    // In equal-width mode, white keys are drawn with a uniform width of 12/7 key
    // widths so they fully cover the keyboard bottom. Black keys keep their
    // original equal-width positions.
    let mut white_expanded = [(0.0f32, 0.0f32); 128];
    if equal_key_width {
        let key_w = width as f32 / 128.0;
        let white_w = key_w * (12.0 / 7.0);
        let mut white_idx = 0usize;
        for key in 0..128u8 {
            if is_black_key(key) {
                continue;
            }
            let x = white_idx as f32 * white_w;
            white_expanded[key as usize] = (x, white_w);
            white_idx += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_key_instance(
        key: u8,
        x: f32,
        w: f32,
        key_top: f32,
        height: f32,
        default_color: (f32, f32, f32),
        corner_radius: f32,
        active_keys: &[bool; 128],
        active_colors: &[[f32; 3]; 128],
    ) -> Option<NoteInstance> {
        if w <= 0.0 {
            return None;
        }
        let (r, g, b) = if active_keys[key as usize] {
            let [cr, cg, cb] = active_colors[key as usize];
            (cr, cg, cb)
        } else {
            default_color
        };
        Some(NoteInstance {
            x,
            y: key_top,
            w,
            h: height,
            rgba_packed: pack_rgba(r, g, b, 1.0),
            props_packed: pack_props(corner_radius, KEY_BORDER_WIDTH),
            velocity: 0,
            flags: 0,
        })
    }

    // White keys first
    for key in 0..128u8 {
        if is_black_key(key) {
            continue;
        }
        let (x, w) = if equal_key_width {
            white_expanded[key as usize]
        } else {
            layouts[key as usize]
        };
        if let Some(inst) = build_key_instance(
            key,
            x,
            w,
            key_top,
            kh,
            WHITE_KEY_COLOR,
            WHITE_KEY_CORNER_RADIUS,
            active_keys,
            active_colors,
        ) {
            out.push(inst);
        }
    }

    // Black keys on top
    for key in 0..128u8 {
        if !is_black_key(key) {
            continue;
        }
        let (x, w) = layouts[key as usize];
        if let Some(inst) = build_key_instance(
            key,
            x,
            w,
            key_top,
            black_h,
            BLACK_KEY_COLOR,
            BLACK_KEY_CORNER_RADIUS,
            active_keys,
            active_colors,
        ) {
            out.push(inst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equal_key_layouts_count() {
        let layouts = compute_key_layouts(1920, true);
        assert_eq!(layouts.len(), 128);
        // Each key must have positive width
        for (i, &(x, w)) in layouts.iter().enumerate() {
            assert!(w >= 1.0, "key {} width {}", i, w);
            assert!(x >= 0.0, "key {} x {}", i, x);
        }
    }

    #[test]
    fn test_equal_key_layouts_sum() {
        let layouts = compute_key_layouts(1920, true);
        // Last key's right edge should be roughly equal to width
        let last_x = layouts[127].0;
        let last_w = layouts[127].1;
        let right_edge = (last_x + last_w) as u32;
        // Allow 1px rounding error
        assert!(
            right_edge == 1920 || right_edge == 1919 || right_edge == 1921,
            "right_edge={}",
            right_edge
        );
    }

    #[test]
    fn test_piano_key_layouts_count() {
        let layouts = compute_key_layouts(1920, false);
        assert_eq!(layouts.len(), 128);
    }

    #[test]
    fn test_piano_key_layouts_white_count() {
        let layouts = compute_key_layouts(1920, false);
        // In a piano-style layout, white keys have larger widths than black keys
        let white_keys: Vec<_> = layouts
            .iter()
            .enumerate()
            .filter(|&(i, _)| !is_black_key(i as u8))
            .collect();
        let black_keys: Vec<_> = layouts
            .iter()
            .enumerate()
            .filter(|&(i, _)| is_black_key(i as u8))
            .collect();
        assert_eq!(white_keys.len(), 75, "there should be 75 white keys");
        assert_eq!(black_keys.len(), 53, "there should be 53 black keys");
        // White keys should be wider than black keys
        for &(_, (_, w_white)) in &white_keys {
            for &(_, (_, w_black)) in &black_keys {
                assert!(
                    w_white > w_black,
                    "white width {} should > black width {}",
                    w_white,
                    w_black
                );
            }
        }
    }

    #[test]
    fn test_is_black_key_consistency() {
        // Verify the is_black_key function returns expected counts
        let black_count = (0..128u8).filter(|&k| is_black_key(k)).count();
        assert_eq!(black_count, 53);
        let white_count = (0..128u8).filter(|&k| !is_black_key(k)).count();
        assert_eq!(white_count, 75);
    }
}
