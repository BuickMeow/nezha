use std::collections::{HashMap, HashSet};

pub(super) struct MidiRenderCache {
    states: HashMap<usize, nezha_renderer::MidiRenderState>,
    uploaded_ids: HashSet<usize>,
}

impl Default for MidiRenderCache {
    fn default() -> Self {
        Self {
            states: HashMap::new(),
            uploaded_ids: HashSet::new(),
        }
    }
}

impl MidiRenderCache {
    pub(super) fn ensure_uploaded(
        &mut self,
        renderer: &mut nezha_renderer::Renderer,
        width: u32,
        midi_idx: usize,
        midi: &dyn nezha_renderer::NoteSource,
        equal_key_width: bool,
    ) {
        if !self.uploaded_ids.contains(&midi_idx) {
            renderer.upload_note_data(midi_idx, midi, width, equal_key_width);
            self.uploaded_ids.insert(midi_idx);
        }
    }

    pub(super) fn state_mut(&mut self, midi_idx: usize) -> &mut nezha_renderer::MidiRenderState {
        self.states.entry(midi_idx).or_default()
    }

    pub(super) fn clear(&mut self, renderer: &mut nezha_renderer::Renderer) {
        self.states.clear();
        self.uploaded_ids.clear();
        renderer.clear_note_data();
    }
}
