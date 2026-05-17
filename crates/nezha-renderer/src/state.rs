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
