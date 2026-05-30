mod audio_store;
mod media_store;
mod midi_store;
mod playback_state;
mod render_settings;

use crate::transport::TimelineState;
use nezha_core::MidiFile;
use std::path::PathBuf;

pub use audio_store::{AudioEntry, AudioStore};
pub use media_store::MediaStore;
pub use midi_store::{MidiEntry, MidiStore};
pub use playback_state::PlaybackState;
pub use render_settings::RenderSettings;

const DEFAULT_DURATION_SECS: f64 = 120.0;

#[derive(Clone, Debug)]
pub struct SoundFontEntry {
    pub path: PathBuf,
}

pub struct ProjectState {
    pub playback: PlaybackState,
    pub midi: MidiStore,
    pub audio: AudioStore,
    pub media: MediaStore,
    pub soundfonts: Vec<SoundFontEntry>,
    pub render: RenderSettings,
    pub timeline_state: TimelineState,
    pub last_error: Option<String>,
}

impl ProjectState {
    pub fn new() -> Self {
        Self {
            playback: PlaybackState::default(),
            midi: MidiStore::default(),
            audio: AudioStore::default(),
            media: MediaStore::default(),
            soundfonts: Vec::new(),
            render: RenderSettings::default(),
            last_error: None,
            timeline_state: TimelineState {
                fps: 60,
                ..Default::default()
            },
        }
    }

    pub fn highlighted_midi(&self) -> Option<&MidiFile> {
        self.midi.highlighted_midi()
    }

    pub fn duration(&self) -> f64 {
        self.timeline_state.data.content_duration() as f64
    }

    pub fn insert_midi(&mut self, path: String, midi: MidiFile) -> usize {
        let duration = midi.duration;
        let idx = self.midi.insert(path, midi, &mut self.timeline_state);
        self.sync_timeline_settings();
        self.timeline_state.data.update_duration(duration as f32);
        self.playback.reset();
        idx
    }

    pub fn remove_midi(&mut self, idx: usize) {
        self.midi.remove(idx, &mut self.timeline_state);
        self.sync_timeline_settings();
        let fallback_duration = self
            .highlighted_midi()
            .map(|m| m.duration)
            .unwrap_or(DEFAULT_DURATION_SECS);
        self.timeline_state
            .data
            .update_duration(fallback_duration as f32);
        self.playback.current_time = self.playback.current_time.min(self.duration());
        self.playback.start = None;
    }

    fn sync_timeline_settings(&mut self) {
        self.timeline_state.fps = self.render.fps;
    }

    pub fn audio_timeline_clips(&self) -> Vec<(usize, f32, f32)> {
        self.timeline_state
            .data
            .tracks
            .iter()
            .filter(|t| t.kind == crate::transport::TrackKind::Audio)
            .flat_map(|t| t.clips.iter())
            .filter_map(|clip| clip.audio_idx.map(|idx| (idx, clip.start, clip.end)))
            .collect()
    }
}
