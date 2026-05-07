use nezha_core::MidiFile;
use crate::app::RenderContext;
use crate::transport::TimelineState;

pub struct ProjectState {
    pub is_playing: bool,
    pub current_time: f32,
    pub duration: f32,
    pub midi_file: Option<MidiFile>,
    pub midi_path: Option<String>,
    pub render_width: u32,
    pub render_height: u32,
    pub fps: u32,
    pub timeline_state: TimelineState,
}

impl ProjectState {
    pub fn new() -> Self {
        let mut timeline_state = TimelineState::default();
        timeline_state.fps = 60;
        Self {
            is_playing: false,
            current_time: 0.0,
            duration: 120.0,
            midi_file: None,
            midi_path: None,
            render_width: 1920,
            render_height: 1080,
            fps: 60,
            timeline_state,
        }
    }

    pub fn load_midi(&mut self,
        path: String,
        render_ctx: &mut RenderContext,
    ) {
        match MidiFile::load(&path) {
            Ok(midi) => {
                self.duration = midi.duration as f32;
                self.midi_path = Some(path);
                self.timeline_state.update_duration(self.duration);
                self.midi_file = Some(midi);
                self.current_time = 0.0;
                render_ctx.reset_midi_state();
            }
            Err(e) => {
                eprintln!("Failed to load MIDI: {}", e);
            }
        }
    }
}
