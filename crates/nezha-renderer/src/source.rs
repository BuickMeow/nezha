use nezha_core::MidiFile;

/// Abstraction over a MIDI note source, decoupling the renderer from any specific format.
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
