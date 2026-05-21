/// Mutable scan state used during rendering to skip already-passed notes.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let state = MidiRenderState::default();
        assert_eq!(state.scan_indices, [0; 128]);
        assert_eq!(state.last_time, -1.0);
        assert_eq!(state.last_scroll_tick, -1.0);
    }

    #[test]
    fn test_reset_after_mutation() {
        let mut state = MidiRenderState::default();
        state.scan_indices[0] = 42;
        state.scan_indices[127] = 99;
        state.last_time = 100.0;
        state.last_scroll_tick = 200.0;

        state.reset();
        assert_eq!(state.scan_indices, [0; 128]);
        assert_eq!(state.last_time, -1.0);
        assert_eq!(state.last_scroll_tick, -1.0);
    }

    #[test]
    fn test_reset_idempotent() {
        let mut state = MidiRenderState::default();
        state.reset();
        // Resetting twice should be fine
        state.reset();
        assert_eq!(state.scan_indices, [0; 128]);
    }
}
