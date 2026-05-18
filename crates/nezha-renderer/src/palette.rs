/// Deterministic multiplier for generating palette hues.
const PALETTE_HUE_MULT: f32 = 0.12345;

/// Saturation used in the generated palette.
const PALETTE_SATURATION: f32 = 0.8;

/// Value (brightness) used in the generated palette.
const PALETTE_VALUE: f32 = 1.0;

/// Generate a deterministic pseudo-random HSV palette with 128 entries.
pub fn random_palette() -> [[f32; 3]; 128] {
    let mut palette = [[0.0f32; 3]; 128];
    for i in 0..128 {
        let hue = ((i as f32 * PALETTE_HUE_MULT) % 1.0) * 360.0;
        let (r, g, b) = hsv_to_rgb(hue, PALETTE_SATURATION, PALETTE_VALUE);
        palette[i] = [r, g, b];
    }
    palette
}

/// Convert HSV (hue: 0-360, saturation: 0-1, value: 0-1) to RGB.
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (r + m, g + m, b + m)
}
