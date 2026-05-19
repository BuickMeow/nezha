use crate::MidiError;
use crate::midi::{LoadProgress, MidiFile, Note};
use crate::time::{DEFAULT_MPQ, TIMECODE_FALLBACK_TPB, ticks_to_seconds};
use std::path::Path;

/// 全局 tempo 事件，按 tick 排序。
#[derive(Clone, Debug)]
struct TempoEvent {
    tick: u32,
    micros_per_quarter: u64,
}

/// 正在演奏中的音符（等待 NoteOff）。
#[derive(Clone, Copy, Debug)]
struct ActiveNote {
    key: u8,
    start_time: f64,
    velocity: u8,
    channel: u8,
    start_tick: u32,
    track: u16,
}

pub struct MidiParser;

impl MidiParser {
    pub fn load(path: impl AsRef<Path>) -> Result<MidiFile, MidiError> {
        Self::load_with_progress(path, |_| {})
    }

    pub fn load_with_progress(
        path: impl AsRef<Path>,
        progress: impl FnMut(LoadProgress),
    ) -> Result<MidiFile, MidiError> {
        let data = std::fs::read(path.as_ref())?;
        Self::parse_bytes_with_progress(&data, progress)
    }

    pub fn parse_bytes_with_progress(
        data: &[u8],
        mut progress: impl FnMut(LoadProgress),
    ) -> Result<MidiFile, MidiError> {
        let smf = midly::Smf::parse(data)?;

        let ticks_per_beat = match smf.header.timing {
            midly::Timing::Metrical(t) => t.as_int() as u32,
            midly::Timing::Timecode(_, _) => TIMECODE_FALLBACK_TPB,
        };

        let tempo_events = Self::collect_tempo_events(&smf.tracks);
        let tempo_segments = Self::build_tempo_segments(tempo_events, ticks_per_beat);

        let mut key_notes: [Vec<Note>; 128] = std::array::from_fn(|_| Vec::new());
        let mut global_duration = 0.0f64;
        let total_tracks = smf.tracks.len();

        for (track_idx, track) in smf.tracks.iter().enumerate() {
            progress(LoadProgress {
                current_track: track_idx + 1,
                total_tracks,
            });
            Self::parse_track(
                track,
                &tempo_segments,
                ticks_per_beat,
                track_idx as u16,
                &mut key_notes,
                &mut global_duration,
            );
        }

        // 每个 key 内按 start 排序
        for notes in &mut key_notes {
            notes.sort_by(|a, b| {
                a.start
                    .partial_cmp(&b.start)
                    .expect("note start times should never be NaN")
            });
        }

        Ok(MidiFile {
            key_notes,
            duration: global_duration,
            ticks_per_beat,
            tempo_segments,
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

    fn build_tempo_segments(
        events: Vec<TempoEvent>,
        ticks_per_beat: u32,
    ) -> Vec<crate::TempoSegment> {
        let mut segments = Vec::new();
        let mut last_tick: u32 = 0;
        let mut last_time: f64 = 0.0;
        let mut last_mpq: u64 = DEFAULT_MPQ;

        if events.is_empty() || events[0].tick > 0 {
            segments.push(crate::TempoSegment {
                start_tick: 0,
                start_time: 0.0,
                micros_per_quarter: DEFAULT_MPQ,
            });
        }

        for ev in events {
            let dtick = ev.tick - last_tick;
            if dtick > 0 {
                last_time += ticks_to_seconds(dtick, ticks_per_beat, last_mpq);
            }
            segments.push(crate::TempoSegment {
                start_tick: ev.tick,
                start_time: last_time,
                micros_per_quarter: ev.micros_per_quarter,
            });
            last_tick = ev.tick;
            last_mpq = ev.micros_per_quarter;
        }
        segments
    }

    fn parse_track(
        track: &midly::Track,
        segments: &[crate::TempoSegment],
        ticks_per_beat: u32,
        track_idx: u16,
        key_notes: &mut [Vec<Note>; 128],
        global_duration: &mut f64,
    ) {
        let mut active_notes: Vec<ActiveNote> = Vec::new();
        let mut current_tick: u32 = 0;
        let mut current_seconds: f64 = 0.0;
        let mut seg_idx: usize = 0;

        for event in track {
            let new_tick = current_tick + event.delta.as_int();
            let delta = new_tick - current_tick;

            if delta > 0 {
                let (new_seconds, new_seg_idx) = Self::advance_time(
                    current_tick,
                    current_seconds,
                    new_tick,
                    seg_idx,
                    segments,
                    ticks_per_beat,
                );
                current_tick = new_tick;
                current_seconds = new_seconds;
                seg_idx = new_seg_idx;
            } else {
                current_tick = new_tick;
            }

            if let midly::TrackEventKind::Midi { channel, message } = event.kind {
                match message {
                    midly::MidiMessage::NoteOn { key, vel } => {
                        let k = key.as_int();
                        let ch = channel.as_int();
                        if vel.as_int() > 0 {
                            active_notes.push(ActiveNote {
                                key: k,
                                start_time: current_seconds,
                                velocity: vel.as_int(),
                                channel: ch,
                                start_tick: current_tick,
                                track: track_idx,
                            });
                        } else {
                            Self::resolve_note_off(
                                k,
                                ch,
                                current_seconds,
                                current_tick,
                                &mut active_notes,
                                key_notes,
                                global_duration,
                            );
                        }
                    }
                    midly::MidiMessage::NoteOff { key, .. } => {
                        let k = key.as_int();
                        let ch = channel.as_int();
                        Self::resolve_note_off(
                            k,
                            ch,
                            current_seconds,
                            current_tick,
                            &mut active_notes,
                            key_notes,
                            global_duration,
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    /// 从当前时间推进到目标 tick，跨 segment 时自动处理 tempo 变化。
    fn advance_time(
        current_tick: u32,
        current_seconds: f64,
        target_tick: u32,
        mut seg_idx: usize,
        segments: &[crate::TempoSegment],
        ticks_per_beat: u32,
    ) -> (f64, usize) {
        let mut tick_cursor = current_tick;
        let mut sec_cursor = current_seconds;

        while seg_idx + 1 < segments.len() && segments[seg_idx + 1].start_tick <= target_tick {
            let boundary = segments[seg_idx + 1].start_tick;
            let d = boundary - tick_cursor;
            sec_cursor += ticks_to_seconds(d, ticks_per_beat, segments[seg_idx].micros_per_quarter);
            tick_cursor = boundary;
            seg_idx += 1;
        }

        let d = target_tick - tick_cursor;
        sec_cursor += ticks_to_seconds(d, ticks_per_beat, segments[seg_idx].micros_per_quarter);

        (sec_cursor, seg_idx)
    }

    fn resolve_note_off(
        key: u8,
        channel: u8,
        end_time: f64,
        end_tick: u32,
        active_notes: &mut Vec<ActiveNote>,
        key_notes: &mut [Vec<Note>; 128],
        global_duration: &mut f64,
    ) {
        if let Some(idx) = active_notes
            .iter()
            .rposition(|n| n.key == key && n.channel == channel)
        {
            let n = active_notes.swap_remove(idx);
            *global_duration = global_duration.max(end_time);
            key_notes[n.key as usize].push(Note {
                key: n.key,
                start: n.start_time,
                end: end_time,
                start_tick: n.start_tick,
                end_tick,
                velocity: n.velocity,
                channel: n.channel,
                track: n.track,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TempoSegment;
    use crate::time::{DEFAULT_MPQ, bpm_from_mpq};

    #[test]
    fn test_ticks_to_seconds_and_back() {
        let dtick = 480;
        let tpb = 480;
        let mpq = DEFAULT_MPQ; // 120 BPM
        let secs = ticks_to_seconds(dtick, tpb, mpq);
        // 480 ticks @ 480 tpb = 1 quarter @ 120 BPM = 0.5s
        assert!((secs - 0.5).abs() < 1e-9);

        let ticks_back = crate::time::seconds_to_ticks(secs, tpb, mpq);
        assert!((ticks_back - dtick as f64).abs() < 1e-9);
    }

    #[test]
    fn test_bpm_from_mpq() {
        assert!((bpm_from_mpq(DEFAULT_MPQ) - 120.0).abs() < 1e-3);
        assert!((bpm_from_mpq(250_000) - 240.0).abs() < 1e-3);
    }

    #[test]
    fn test_build_tempo_segments_empty() {
        let segments = MidiParser::build_tempo_segments(vec![], 480);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start_tick, 0);
        assert_eq!(segments[0].micros_per_quarter, DEFAULT_MPQ);
    }

    #[test]
    fn test_build_tempo_segments_single_event_at_zero() {
        let events = vec![TempoEvent {
            tick: 0,
            micros_per_quarter: 250_000,
        }];
        let segments = MidiParser::build_tempo_segments(events, 480);
        // 第一个 event 在 tick 0，不需要额外的默认 segment
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].micros_per_quarter, 250_000);
    }

    #[test]
    fn test_build_tempo_segments_two_events() {
        let events = vec![
            TempoEvent {
                tick: 0,
                micros_per_quarter: DEFAULT_MPQ,
            },
            TempoEvent {
                tick: 480,
                micros_per_quarter: 250_000,
            },
        ];
        let segments = MidiParser::build_tempo_segments(events, 480);
        assert_eq!(segments.len(), 2);
        // 第二个 segment 从 tick 480 开始
        assert_eq!(segments[1].start_tick, 480);
        // 480 ticks @ 480 tpb @ 120 BPM = 0.5s
        assert!((segments[1].start_time - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_midi_file_tick_at_time_no_tempo() {
        let midi = MidiFile {
            key_notes: std::array::from_fn(|_| Vec::new()),
            duration: 10.0,
            ticks_per_beat: 480,
            tempo_segments: vec![],
        };
        // 无 tempo 时默认 120 BPM：1s = 2 beats = 960 ticks
        assert!((midi.tick_at_time(1.0) - 960.0).abs() < 1e-6);
        assert!((midi.tick_at_time(0.5) - 480.0).abs() < 1e-6);
    }

    #[test]
    fn test_midi_file_tick_at_time_with_tempo() {
        let midi = MidiFile {
            key_notes: std::array::from_fn(|_| Vec::new()),
            duration: 10.0,
            ticks_per_beat: 480,
            tempo_segments: vec![
                TempoSegment {
                    start_tick: 0,
                    start_time: 0.0,
                    micros_per_quarter: DEFAULT_MPQ,
                },
                TempoSegment {
                    start_tick: 480,
                    start_time: 0.5,
                    micros_per_quarter: 250_000, // 240 BPM，快一倍
                },
            ],
        };
        // 在 120 BPM 段：0.5s = 480 ticks
        assert!((midi.tick_at_time(0.5) - 480.0).abs() < 1e-6);
        // 在 240 BPM 段：再过 0.5s = 960 ticks，总计 1440
        assert!((midi.tick_at_time(1.0) - 1440.0).abs() < 1e-6);
    }

    #[test]
    fn test_midi_file_bpm_at_time() {
        let midi = MidiFile {
            key_notes: std::array::from_fn(|_| Vec::new()),
            duration: 10.0,
            ticks_per_beat: 480,
            tempo_segments: vec![
                TempoSegment {
                    start_tick: 0,
                    start_time: 0.0,
                    micros_per_quarter: DEFAULT_MPQ,
                },
                TempoSegment {
                    start_tick: 480,
                    start_time: 0.5,
                    micros_per_quarter: 250_000,
                },
            ],
        };
        assert!((midi.bpm_at_time(0.0) - 120.0).abs() < 1e-3);
        assert!((midi.bpm_at_time(0.5) - 240.0).abs() < 1e-3);
    }

    #[test]
    fn test_is_black_key() {
        use crate::time::is_black_key;
        assert!(!is_black_key(0)); // C
        assert!(is_black_key(1)); // C#
        assert!(is_black_key(3)); // D#
        assert!(!is_black_key(4)); // E
        assert!(!is_black_key(12)); // 下一个 C
    }
}
