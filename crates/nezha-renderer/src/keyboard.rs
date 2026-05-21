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
        /// Width of black keys relative to white keys.
        const BLACK_KEY_WIDTH_RATIO: f64 = 0.65;
        /// Horizontal offset to center black keys over the white-key boundary.
        const BLACK_KEY_OFFSET_RATIO: f64 = 0.5;
        let black_width = white_width * BLACK_KEY_WIDTH_RATIO;
        let mut white_count = 0usize;
        for key in 0..128u8 {
            if is_black_key(key) {
                let x = (white_count as f64 * white_width - black_width * BLACK_KEY_OFFSET_RATIO)
                    .round() as f32;
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

/// Build vertex instances for the on-screen piano keyboard.
///
/// `active_keys` / `active_colors` should reflect the top-most currently playing
/// note per key, already resolved by the main waterfall scan.
pub(crate) fn append_keyboard_instances(
    layouts: &[(f32, f32)],
    height: u32,
    keyboard_height: f32,
    active_keys: &[bool; 128],
    active_colors: &[[f32; 3]; 128],
    out: &mut Vec<NoteInstance>,
) {
    let kh = keyboard_height.max(1.0);
    let key_top = height as f32 - kh;
    let black_h = kh * BLACK_KEY_HEIGHT_RATIO;
    out.reserve(128);

    fn build_key_instance(
        key: u8,
        layouts: &[(f32, f32)],
        key_top: f32,
        height: f32,
        default_color: (f32, f32, f32),
        corner_radius: f32,
        active_keys: &[bool; 128],
        active_colors: &[[f32; 3]; 128],
    ) -> Option<NoteInstance> {
        let (x, w) = layouts[key as usize];
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
        if let Some(inst) = build_key_instance(
            key,
            &layouts,
            key_top,
            kh,
            WHITE_KEY_COLOR,
            WHITE_KEY_CORNER_RADIUS,
            &active_keys,
            &active_colors,
        ) {
            out.push(inst);
        }
    }

    // Black keys on top
    for key in 0..128u8 {
        if !is_black_key(key) {
            continue;
        }
        if let Some(inst) = build_key_instance(
            key,
            &layouts,
            key_top,
            black_h,
            BLACK_KEY_COLOR,
            BLACK_KEY_CORNER_RADIUS,
            &active_keys,
            &active_colors,
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
