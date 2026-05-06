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
pub struct MidiFile {
    /// 按 key 分组的音符，key_notes[i] 表示 MIDI key=i 的所有音符，已按 start 排序
    pub key_notes: [Vec<Note>; 128],
    pub duration: f32,
}

impl MidiFile {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        let data = std::fs::read(path.as_ref()).map_err(|e| e.to_string())?;
        let smf = midly::Smf::parse(&data).map_err(|e| e.to_string())?;

        let ticks_per_beat = match smf.header.timing {
            midly::Timing::Metrical(t) => t.as_int() as u32,
            midly::Timing::Timecode(_, _) => 480,
        };

        // ── 1. 收集全局 tempo 事件 ──
        let mut tempo_events = Vec::new();
        for track in &smf.tracks {
            let mut tick: u32 = 0;
            for event in track {
                tick += event.delta.as_int();
                if let midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(us)) = event.kind {
                    tempo_events.push((tick, us.as_int()));
                }
            }
        }
        tempo_events.sort_by_key(|(t, _)| *t);
        tempo_events.dedup_by_key(|(t, _)| *t);

        // ── 2. 构建 tempo 段列表 ──
        let mut tempo_segments: Vec<(u32, u64)> = Vec::new();
        if tempo_events.is_empty() || tempo_events[0].0 > 0 {
            tempo_segments.push((0, 500_000));
        }
        for (tick, us) in tempo_events {
            tempo_segments.push((tick, us as u64));
        }

        // ── 3. 解析音符，按 key 分组 ──
        let mut key_notes: [Vec<Note>; 128] = std::array::from_fn(|_| Vec::new());
        let mut global_duration = 0.0f32;

        for track in &smf.tracks {
            let mut active_notes: Vec<(u8, f32, u8, u8)> = Vec::new();
            let mut current_tick: u32 = 0;
            let mut current_seconds: f64 = 0.0;
            let mut seg_idx: usize = 0;

            for event in track {
                let new_tick = current_tick + event.delta.as_int();
                let delta = new_tick - current_tick;

                if delta > 0 {
                    let mut tick_cursor = current_tick;
                    let mut sec_cursor = current_seconds;

                    while seg_idx + 1 < tempo_segments.len()
                        && tempo_segments[seg_idx + 1].0 <= new_tick
                    {
                        let boundary = tempo_segments[seg_idx + 1].0;
                        let d = boundary - tick_cursor;
                        sec_cursor += (d as u64 * tempo_segments[seg_idx].1) as f64
                            / (ticks_per_beat as f64 * 1_000_000.0);
                        tick_cursor = boundary;
                        seg_idx += 1;
                    }

                    let d = new_tick - tick_cursor;
                    sec_cursor += (d as u64 * tempo_segments[seg_idx].1) as f64
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
                                active_notes.push((k, current_seconds as f32, vel.as_int(), ch));
                            } else {
                                if let Some(idx) = active_notes
                                    .iter()
                                    .rposition(|(ak, _, _, ach)| *ak == k && *ach == ch)
                                {
                                    let (k, start, velocity, ch) = active_notes.swap_remove(idx);
                                    let end = current_seconds as f32;
                                    global_duration = global_duration.max(end);
                                    key_notes[k as usize].push(Note {
                                        key: k, start, end, velocity, channel: ch,
                                    });
                                }
                            }
                        }
                        midly::MidiMessage::NoteOff { key, .. } => {
                            let k = key.as_int();
                            let ch = channel.as_int();
                            if let Some(idx) = active_notes
                                .iter()
                                .rposition(|(ak, _, _, ach)| *ak == k && *ach == ch)
                            {
                                let (k, start, velocity, ch) = active_notes.swap_remove(idx);
                                let end = current_seconds as f32;
                                global_duration = global_duration.max(end);
                                key_notes[k as usize].push(Note {
                                    key: k, start, end, velocity, channel: ch,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // 每个 key 内按 start 排序
        for notes in &mut key_notes {
            notes.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
        }

        Ok(MidiFile {
            key_notes,
            duration: global_duration,
        })
    }
}
