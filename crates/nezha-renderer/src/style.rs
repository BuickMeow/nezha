use nezha_core::MidiFile;

/// 音符数据源抽象，解耦 renderer 与具体 MIDI 格式
pub trait NoteSource {
    fn key_notes(&self, key: u8) -> &[nezha_core::Note];
    fn duration(&self) -> f64;
    fn ticks_per_beat(&self) -> Option<u32> {
        None
    }
    fn tick_at_time(&self, _time: f64) -> Option<f64> {
        None
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RenderMode {
    TimeBased,
    TickBased,
}

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

pub fn random_palette() -> [[f32; 3]; 128] {
    let mult = 0.12345f32;
    let mut palette = [[0.0f32; 3]; 128];
    for i in 0..128 {
        let hue = ((i as f32 * mult) % 1.0) * 360.0;
        let (r, g, b) = hsv_to_rgb(hue, 0.8, 1.0);
        palette[i] = [r, g, b];
    }
    palette
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
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

impl NoteSource for MidiFile {
    fn key_notes(&self, key: u8) -> &[nezha_core::Note] {
        &self.key_notes[key as usize]
    }
    fn duration(&self) -> f64 {
        self.duration
    }
    fn ticks_per_beat(&self) -> Option<u32> {
        Some(self.ticks_per_beat)
    }
    fn tick_at_time(&self, time: f64) -> Option<f64> {
        Some(self.tick_at_time(time))
    }
}

pub struct MidiRenderState {
    pub(crate) scan_indices: [usize; 128],
    pub(crate) last_time: f64,
    pub(crate) last_scroll_tick: f64,
}

impl Default for MidiRenderState {
    fn default() -> Self {
        Self {
            scan_indices: [0; 128],
            last_time: -1.0,
            last_scroll_tick: -1.0,
        }
    }
}

impl MidiRenderState {
    pub fn reset(&mut self) {
        self.scan_indices = [0; 128];
        self.last_time = -1.0;
        self.last_scroll_tick = -1.0;
    }
}
