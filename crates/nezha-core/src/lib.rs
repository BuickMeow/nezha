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
    pub key: u8,         // 0-127 MIDI note number
    pub start: f64,      // seconds
    pub end: f64,        // seconds
    pub start_tick: u32, // absolute MIDI tick
    pub end_tick: u32,   // absolute MIDI tick
    pub velocity: u8,    // 0-127
    pub channel: u8,
    pub track: u16, // MIDI track index (0-based)
}

#[derive(Clone, Debug)]
pub struct MidiFile {
    /// 按 key 分组的音符，key_notes[i] 表示 MIDI key=i 的所有音符，已按 start 排序
    pub key_notes: [Vec<Note>; 128],
    pub duration: f64,
    pub ticks_per_beat: u32,
    /// tempo 区间，按 start_tick / start_time 排序
    pub tempo_segments: Vec<TempoSegment>,
}

/// 全局 tempo 事件，按 tick 排序
#[derive(Clone, Debug)]
struct TempoEvent {
    tick: u32,
    micros_per_quarter: u64,
}

/// 一段连续的 tempo 区间
#[derive(Clone, Debug)]
pub struct TempoSegment {
    pub start_tick: u32,
    pub start_time: f64,
    pub micros_per_quarter: u64,
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
        let tempo_segments = Self::build_tempo_segments(tempo_events, ticks_per_beat);

        let mut key_notes: [Vec<Note>; 128] = std::array::from_fn(|_| Vec::new());
        let mut global_duration = 0.0f64;

        for (track_idx, track) in smf.tracks.iter().enumerate() {
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
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        // OR (Overlap Removal): 清理同一 key 内重叠的音符。
        // 参考 C# 实现：相邻音符比较，处理部分重叠和同起点情况。
        for notes in &mut key_notes {
            if notes.len() < 2 {
                continue;
            }
            for i in 0..notes.len() - 1 {
                let (left, right) = notes.split_at_mut(i + 1);
                let curr = &mut left[i];
                let next = &right[0];
                // Case 1: curr 先开始，尾部伸入 next 区间 → 截断 curr
                if curr.start < next.start && curr.end > next.start && curr.end < next.end {
                    curr.end = next.start;
                    curr.end_tick = next.start_tick;
                }
                // Case 2: 同时开始，curr 结束不晚于 next → 移除 curr（设为零长）
                else if curr.start == next.start && curr.end <= next.end {
                    curr.end = curr.start;
                    curr.end_tick = curr.start_tick;
                }
            }
            // 过滤 OR 产生的零长度音符
            notes.retain(|n| n.end > n.start);
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

    fn build_tempo_segments(events: Vec<TempoEvent>, ticks_per_beat: u32) -> Vec<TempoSegment> {
        let mut segments = Vec::new();
        let mut last_tick: u32 = 0;
        let mut last_time: f64 = 0.0;
        let mut last_mpq: u64 = 500_000;

        if events.is_empty() || events[0].tick > 0 {
            segments.push(TempoSegment {
                start_tick: 0,
                start_time: 0.0,
                micros_per_quarter: 500_000,
            });
        }

        for ev in events {
            let dtick = ev.tick - last_tick;
            if dtick > 0 {
                last_time +=
                    (dtick as u64 * last_mpq) as f64 / (ticks_per_beat as f64 * 1_000_000.0);
            }
            segments.push(TempoSegment {
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
        segments: &[TempoSegment],
        ticks_per_beat: u32,
        track_idx: u16,
        key_notes: &mut [Vec<Note>; 128],
        global_duration: &mut f64,
    ) {
        let mut active_notes: Vec<(u8, f64, u8, u8, u32, u16)> = Vec::new();
        let mut current_tick: u32 = 0;
        let mut current_seconds: f64 = 0.0;
        let mut seg_idx: usize = 0;

        for event in track {
            let new_tick = current_tick + event.delta.as_int();
            let delta = new_tick - current_tick;

            if delta > 0 {
                let mut tick_cursor = current_tick;
                let mut sec_cursor = current_seconds;

                while seg_idx + 1 < segments.len() && segments[seg_idx + 1].start_tick <= new_tick {
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
                            active_notes.push((
                                k,
                                current_seconds,
                                vel.as_int(),
                                ch,
                                current_tick,
                                track_idx,
                            ));
                        } else {
                            Self::resolve_note_off(
                                k,
                                ch,
                                current_seconds,
                                current_tick,
                                track_idx,
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
                            track_idx,
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

    /// 将秒时间转换为对应的 MIDI tick（考虑 tempo 变化）
    pub fn tick_at_time(&self, time: f64) -> f64 {
        if self.tempo_segments.is_empty() {
            return time * self.ticks_per_beat as f64 * 2.0; // 120BPM default
        }

        // 找到包含该时间的 segment
        let seg_idx = self
            .tempo_segments
            .iter()
            .rposition(|s| s.start_time <= time)
            .unwrap_or(0);
        let seg = &self.tempo_segments[seg_idx];

        let dt = time - seg.start_time;
        seg.start_tick as f64
            + dt * self.ticks_per_beat as f64 * 1_000_000.0 / seg.micros_per_quarter as f64
    }

    /// 获取指定时间的 BPM
    pub fn bpm_at_time(&self, time: f64) -> f32 {
        if self.tempo_segments.is_empty() {
            return 120.0;
        }
        let seg_idx = self
            .tempo_segments
            .iter()
            .rposition(|s| s.start_time <= time)
            .unwrap_or(0);
        let mpq = self.tempo_segments[seg_idx].micros_per_quarter;
        (60_000_000.0 / mpq as f64) as f32
    }

    fn resolve_note_off(
        key: u8,
        channel: u8,
        end_time: f64,
        end_tick: u32,
        _track_idx: u16,
        active_notes: &mut Vec<(u8, f64, u8, u8, u32, u16)>,
        key_notes: &mut [Vec<Note>; 128],
        global_duration: &mut f64,
    ) {
        if let Some(idx) = active_notes
            .iter()
            .rposition(|(ak, _, _, ach, _, _)| *ak == key && *ach == channel)
        {
            let (k, start, velocity, ch, start_tick, trk) = active_notes.swap_remove(idx);
            *global_duration = global_duration.max(end_time);
            key_notes[k as usize].push(Note {
                key: k,
                start,
                end: end_time,
                start_tick,
                end_tick,
                velocity,
                channel: ch,
                track: trk,
            });
        }
    }
}
