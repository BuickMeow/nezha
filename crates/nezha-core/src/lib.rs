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

#[derive(Clone, Debug)]
struct TempoEvent {
    tick: u32,
    microseconds_per_quarter: u32,
}

impl MidiFile {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let data = std::fs::read(path.as_ref()).map_err(|e| e.to_string())?;
        let smf = midly::Smf::parse(&data).map_err(|e| e.to_string())?;

        let ticks_per_beat = match smf.header.timing {
            midly::Timing::Metrical(t) => t.as_int() as u32,
            midly::Timing::Timecode(_, _) => 480,
        };

        // 收集全局 tempo 变化事件
        let mut tempo_events = Vec::new();
        for track in &smf.tracks {
            let mut tick: u32 = 0;
            for event in track {
                tick += event.delta.as_int();
                if let midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(us_per_qn)) = event.kind {
                    tempo_events.push(TempoEvent {
                        tick,
                        microseconds_per_quarter: us_per_qn.as_int(),
                    });
                }
            }
        }
        tempo_events.sort_by_key(|e| e.tick);

        // 如果没有 tempo 事件，默认 120 BPM = 500000 µs/quarter
        if tempo_events.is_empty() {
            tempo_events.push(TempoEvent {
                tick: 0,
                microseconds_per_quarter: 500_000,
            });
        }

        let tick_to_seconds = |target_tick: u32| -> f32 {
            let mut seconds: f64 = 0.0;
            let mut current_tick: u32 = 0;
            let mut current_us_per_qn: u64 = tempo_events[0].microseconds_per_quarter as u64;

            for event in &tempo_events[1..] {
                if event.tick >= target_tick {
                    break;
                }
                let delta_ticks = event.tick - current_tick;
                let delta_seconds = (delta_ticks as u64 * current_us_per_qn) as f64
                    / (ticks_per_beat as f64 * 1_000_000.0);
                seconds += delta_seconds;
                current_tick = event.tick;
                current_us_per_qn = event.microseconds_per_quarter as u64;
            }

            let delta_ticks = target_tick - current_tick;
            let delta_seconds = (delta_ticks as u64 * current_us_per_qn) as f64
                / (ticks_per_beat as f64 * 1_000_000.0);
            seconds += delta_seconds;

            seconds as f32
        };

        let mut tracks = Vec::new();
        let mut global_duration = 0.0f32;

        for track in &smf.tracks {
            let mut notes: Vec<Note> = Vec::new();
            let mut active_notes: Vec<(u8, u32, u8, u8)> = Vec::new();
            let mut current_tick: u32 = 0;

            for event in track {
                current_tick += event.delta.as_int();
                if let midly::TrackEventKind::Midi { channel, message } = event.kind {
                    match message {
                        midly::MidiMessage::NoteOn { key, vel } => {
                            if vel.as_int() > 0 {
                                active_notes.push((key.as_int(), current_tick, vel.as_int(), channel.as_int()));
                            } else {
                                if let Some(idx) = active_notes.iter().rposition(|(k, _, _, ch)| *k == key.as_int() && *ch == channel.as_int()) {
                                    let (k, start_tick, velocity, ch) = active_notes.swap_remove(idx);
                                    let start = tick_to_seconds(start_tick);
                                    let end = tick_to_seconds(current_tick);
                                    let note = Note { key: k, start, end, velocity, channel: ch };
                                    global_duration = global_duration.max(end);
                                    notes.push(note);
                                }
                            }
                        }
                        midly::MidiMessage::NoteOff { key, .. } => {
                            if let Some(idx) = active_notes.iter().rposition(|(k, _, _, ch)| *k == key.as_int() && *ch == channel.as_int()) {
                                let (k, start_tick, velocity, ch) = active_notes.swap_remove(idx);
                                let start = tick_to_seconds(start_tick);
                                let end = tick_to_seconds(current_tick);
                                let note = Note { key: k, start, end, velocity, channel: ch };
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
}

impl MidiFile {
    pub fn all_notes(&self) -> Vec<&Note> {
        self.tracks.iter().flat_map(|t| t.notes.iter()).collect()
    }
}
