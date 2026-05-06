use std::path::Path;

pub struct RenderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Note {
    pub key: u8,        // 0-127 MIDI note number
    pub start: f32,     // seconds
    pub end: f32,       // seconds
    pub velocity: u8,   // 0-127
    pub channel: u8,
}

#[derive(Clone, Debug)]
pub struct MidiTrack {
    pub notes: Vec<Note>,
}

#[derive(Clone, Debug)]
pub struct MidiFile {
    pub tracks: Vec<MidiTrack>,
    pub duration: f32,
}

impl MidiFile {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let data = std::fs::read(path.as_ref()).map_err(|e| e.to_string())?;
        let smf = midly::Smf::parse(&data).map_err(|e| e.to_string())?;

        let mut tracks = Vec::new();
        let mut global_duration = 0.0f32;

        for track in &smf.tracks {
            let mut notes: Vec<Note> = Vec::new();
            let mut active_notes: Vec<(u8, u32, u8, u8)> = Vec::new(); // (key, start_tick, velocity, channel)
            let mut current_tick: u32 = 0;

            for event in track {
                current_tick += event.delta.as_int();
                if let midly::TrackEventKind::Midi { channel, message } = event.kind {
                    match message {
                        midly::MidiMessage::NoteOn { key, vel } => {
                            if vel.as_int() > 0 {
                                active_notes.push((key.as_int(), current_tick, vel.as_int(), channel.as_int()));
                            } else {
                                // NoteOn with velocity 0 is equivalent to NoteOff
                                if let Some(idx) = active_notes.iter().rposition(|(k, _, _, ch)| *k == key.as_int() && *ch == channel.as_int()) {
                                    let (k, start_tick, velocity, ch) = active_notes.swap_remove(idx);
                                    let start = Self::tick_to_seconds(start_tick, &smf.header);
                                    let end = Self::tick_to_seconds(current_tick, &smf.header);
                                    let note = Note {
                                        key: k,
                                        start,
                                        end,
                                        velocity,
                                        channel: ch,
                                    };
                                    global_duration = global_duration.max(end);
                                    notes.push(note);
                                }
                            }
                        }
                        midly::MidiMessage::NoteOff { key, .. } => {
                            if let Some(idx) = active_notes.iter().rposition(|(k, _, _, ch)| *k == key.as_int() && *ch == channel.as_int()) {
                                let (k, start_tick, velocity, ch) = active_notes.swap_remove(idx);
                                let start = Self::tick_to_seconds(start_tick, &smf.header);
                                let end = Self::tick_to_seconds(current_tick, &smf.header);
                                let note = Note {
                                    key: k,
                                    start,
                                    end,
                                    velocity,
                                    channel: ch,
                                };
                                global_duration = global_duration.max(end);
                                notes.push(note);
                            }
                        }
                        _ => {}
                    }
                }
            }

            if !notes.is_empty() {
                notes.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
                tracks.push(MidiTrack { notes });
            }
        }

        Ok(MidiFile {
            tracks,
            duration: global_duration,
        })
    }

    fn tick_to_seconds(tick: u32, header: &midly::Header) -> f32 {
        // Simplified: assume 120 BPM, 480 ticks per beat
        // In a real implementation, you'd parse tempo meta events
        let ticks_per_beat = match header.timing {
            midly::Timing::Metrical(t) => t.as_int() as f32,
            midly::Timing::Timecode(_, _) => 480.0,
        };
        let bpm = 120.0;
        let seconds_per_beat = 60.0 / bpm;
        tick as f32 / ticks_per_beat * seconds_per_beat
    }
}

impl MidiFile {
    pub fn all_notes(&self) -> Vec<&Note> {
        self.tracks.iter().flat_map(|t| t.notes.iter()).collect()
    }
}
