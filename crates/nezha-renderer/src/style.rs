use crate::palette::random_palette;

/// How the renderer maps time to vertical position.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RenderMode {
    /// Scroll based on wall-clock time (seconds).
    TimeBased,
    /// Scroll based on MIDI ticks.
    TickBased,
}

/// Visual style configuration for a render pass.
#[derive(Clone)]
pub struct RenderStyle {
    pub render_mode: RenderMode,
    pub border_width: f32,
    pub rounding: f32,
    pub track_index: usize,
    pub palette: [[f32; 3]; 128],
    pub background: [f64; 4],
    pub equal_key_width: bool,
    pub keyboard_height: f32,
}

impl Default for RenderStyle {
    fn default() -> Self {
        Self {
            render_mode: RenderMode::TimeBased,
            border_width: 0.1,
            rounding: 0.0,
            track_index: 0,
            palette: random_palette(),
            background: [0.0, 0.0, 0.0, 1.0],
            equal_key_width: true,
            keyboard_height: 0.0,
        }
    }
}
