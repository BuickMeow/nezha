use crate::source::NoteSource;
use crate::state::MidiRenderState;
use crate::style::RenderMode;

use super::Renderer;

impl Renderer {
    pub(super) fn upload_scans(
        &mut self,
        midi: &dyn NoteSource,
        state: &mut MidiRenderState,
        time: f64,
        scroll_tick: f64,
        mode: RenderMode,
    ) {
        profile_scope!("scans");
        Self::advance_scan_indices(midi, state, time, scroll_tick, mode);
        let scans_u32: [u32; 128] = std::array::from_fn(|i| state.scan_indices[i] as u32);
        self.queue
            .write_buffer(&self.compute.scan_buffer, 0, bytemuck::bytes_of(&scans_u32));
    }

    pub(super) fn advance_scan_indices(
        midi: &dyn NoteSource,
        state: &mut MidiRenderState,
        time: f64,
        scroll_tick: f64,
        mode: RenderMode,
    ) {
        match mode {
            RenderMode::TimeBased => {
                if time < state.last_time {
                    state.scan_indices = [0; 128];
                }
                state.last_time = time;
                for key in 0..128u8 {
                    let notes = midi.key_notes(key);
                    if notes.is_empty() {
                        continue;
                    }
                    let mut scan = state.scan_indices[key as usize];
                    while scan < notes.len() && notes[scan].end <= time {
                        scan += 1;
                    }
                    state.scan_indices[key as usize] = scan;
                }
            }
            RenderMode::TickBased => {
                if scroll_tick < state.last_scroll_tick {
                    state.scan_indices = [0; 128];
                }
                state.last_scroll_tick = scroll_tick;
                for key in 0..128u8 {
                    let notes = midi.key_notes(key);
                    if notes.is_empty() {
                        continue;
                    }
                    let mut scan = state.scan_indices[key as usize];
                    while scan < notes.len() && (notes[scan].end_tick as f64) <= scroll_tick {
                        scan += 1;
                    }
                    state.scan_indices[key as usize] = scan;
                }
            }
        }
    }
}
