use std::path::Path;

#[derive(Debug)]
pub enum MidiError {
    Io(std::io::Error),
    Parse(midly::Error),
}

impl std::fmt::Display for MidiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MidiError::Io(e) => write!(f, "IO error: {}", e),
            MidiError::Parse(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for MidiError {}

impl From<std::io::Error> for MidiError {
    fn from(e: std::io::Error) -> Self {
        MidiError::Io(e)
    }
}

impl From<midly::Error> for MidiError {
    fn from(e: midly::Error) -> Self {
        MidiError::Parse(e)
    }
}

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
    pub key: u8,              // 0-127 MIDI note number
    pub start: f64,           // seconds
    pub end: f64,             // seconds
    pub start_tick: u32,      // absolute MIDI tick
    pub end_tick: u32,        // absolute MIDI tick
    pub velocity: u8,         // 0-127
    pub channel: u8,
}

#[derive(Clone, Debug)]
pub struct MidiFile {
    /// 按 key 分组的音符，key_notes[i] 表示 MIDI key=i 的所有音符，已按 start 排序
    pub key_notes: [Vec<Note>; 128],
    pub duration: f64,
    pub ticks_per_beat: u32,
}

/// 全局 tempo 事件，按 tick 排序
#[derive(Clone, Debug)]
struct TempoEvent {
    tick: u32,
    micros_per_quarter: u64,
}

/// 一段连续的 tempo 区间
#[derive(Clone, Debug)]
struct TempoSegment {
    start_tick: u32,
    micros_per_quarter: u64,
}

impl MidiFile {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, MidiError> {
        let data = std::fs::read(path.as_ref())?;
        let smf = midly::Smf::parse(&data)?;

        let ticks_per_beat = match smf.header.timing {
            midly::Timing::Metrical(t) => t.as_int() as u32,
            midly::Timing::Timecode(_, _) => 480,
        };

        let tempo_events = Self::collect_tempo_events(&smf.tracks);
        let tempo_segments = Self::build_tempo_segments(tempo_events);

        let mut key_notes: [Vec<Note>; 128] = std::array::from_fn(|_| Vec::new());
        let mut global_duration = 0.0f64;

        for track in &smf.tracks {
            Self::parse_track(track, &tempo_segments, ticks_per_beat, &mut key_notes, &mut global_duration);
        }

        // 每个 key 内按 start 排序
        for notes in &mut key_notes {
            notes.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
        }

        Ok(MidiFile {
            key_notes,
            duration: global_duration,
            ticks_per_beat,
        })
    }

    fn collect_tempo_events(tracks: &[midly::Track]) -> Vec<TempoEvent> {
        let mut events = Vec::new();
        for track in tracks {
            let mut tick: u32 = 0;
            for event in track {
                tick += event.delta.as_int();
                if let midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(us)) = event.kind {
                    events.push(TempoEvent {
                        tick,
                        micros_per_quarter: us.as_int() as u64,
                    });
                }
            }
        }
        events.sort_by_key(|e| e.tick);
        events.dedup_by_key(|e| e.tick);
        events
    }

    fn build_tempo_segments(events: Vec<TempoEvent>) -> Vec<TempoSegment> {
        let mut segments = Vec::new();
        if events.is_empty() || events[0].tick > 0 {
            segments.push(TempoSegment {
                start_tick: 0,
                micros_per_quarter: 500_000,
            });
        }
        for ev in events {
            segments.push(TempoSegment {
                start_tick: ev.tick,
                micros_per_quarter: ev.micros_per_quarter,
            });
        }
        segments
    }

    fn parse_track(
        track: &midly::Track,
        segments: &[TempoSegment],
        ticks_per_beat: u32,
        key_notes: &mut [Vec<Note>; 128],
        global_duration: &mut f64,
    ) {
        let mut active_notes: Vec<(u8, f64, u8, u8, u32)> = Vec::new();
        let mut current_tick: u32 = 0;
        let mut current_seconds: f64 = 0.0;
        let mut seg_idx: usize = 0;

        for event in track {
            let new_tick = current_tick + event.delta.as_int();
            let delta = new_tick - current_tick;

            if delta > 0 {
                let mut tick_cursor = current_tick;
                let mut sec_cursor = current_seconds;

                while seg_idx + 1 < segments.len()
                    && segments[seg_idx + 1].start_tick <= new_tick
                {
                    let boundary = segments[seg_idx + 1].start_tick;
                    let d = boundary - tick_cursor;
                    sec_cursor += (d as u64 * segments[seg_idx].micros_per_quarter) as f64
                        / (ticks_per_beat as f64 * 1_000_000.0);
                    tick_cursor = boundary;
                    seg_idx += 1;
                }

                let d = new_tick - tick_cursor;
                sec_cursor += (d as u64 * segments[seg_idx].micros_per_quarter) as f64
                    / (ticks_per_beat as f64 * 1_000_000.0);

                current_tick = new_tick;
                current_seconds = sec_cursor;
            } else {
                current_tick = new_tick;
            }

            if let midly::TrackEventKind::Midi { channel, message } = event.kind {
                match message {
                    midly::MidiMessage::NoteOn { key, vel } => {
                        let k = key.as_int();
                        let ch = channel.as_int();
                        if vel.as_int() > 0 {
                            active_notes.push((k, current_seconds, vel.as_int(), ch, current_tick));
                        } else {
                            Self::resolve_note_off(k, ch, current_seconds, current_tick, &mut active_notes, key_notes, global_duration);
                        }
                    }
                    midly::MidiMessage::NoteOff { key, .. } => {
                        let k = key.as_int();
                        let ch = channel.as_int();
                        Self::resolve_note_off(k, ch, current_seconds, current_tick, &mut active_notes, key_notes, global_duration);
                    }
                    _ => {}
                }
            }
        }
    }

    fn resolve_note_off(
        key: u8,
        channel: u8,
        end_time: f64,
        end_tick: u32,
        active_notes: &mut Vec<(u8, f64, u8, u8, u32)>,
        key_notes: &mut [Vec<Note>; 128],
        global_duration: &mut f64,
    ) {
        if let Some(idx) = active_notes
            .iter()
            .rposition(|(ak, _, _, ach, _)| *ak == key && *ach == channel)
        {
            let (k, start, velocity, ch, start_tick) = active_notes.swap_remove(idx);
            *global_duration = global_duration.max(end_time);
            key_notes[k as usize].push(Note {
                key: k,
                start,
                end: end_time,
                start_tick,
                end_tick,
                velocity,
                channel: ch,
            });
        }
    }
}
