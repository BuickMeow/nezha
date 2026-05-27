use crate::TempoSegment;
use crate::error::MidiError;
use crate::parser::MidiParser;
use crate::time::{DEFAULT_BPM, DEFAULT_MPQ, bpm_from_mpq, seconds_to_ticks};
use std::path::Path;

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
    /// 总音符数。
    pub note_count: u64,
    /// 最后一个音符结束时的 tick（总 tick 长度）。
    pub tick_length: u64,
    /// 拍号分子（如 4/4 中的 4）。
    pub time_sig_numerator: u8,
    /// 拍号分母（如 4/4 中的 4，实际值为 2^4 = 16）。
    pub time_sig_denominator: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct LoadProgress {
    pub current_track: usize,
    pub total_tracks: usize,
}

impl MidiFile {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, MidiError> {
        MidiParser::load(path)
    }

    pub fn load_with_progress(
        path: impl AsRef<Path>,
        progress: impl FnMut(LoadProgress),
    ) -> Result<Self, MidiError> {
        MidiParser::load_with_progress(path, progress)
    }

    pub fn load_from_bytes(data: &[u8]) -> Result<Self, MidiError> {
        MidiParser::parse_bytes_with_progress(data, |_| {})
    }

    pub fn load_from_bytes_with_progress(
        data: &[u8],
        progress: impl FnMut(LoadProgress),
    ) -> Result<Self, MidiError> {
        MidiParser::parse_bytes_with_progress(data, progress)
    }

    /// 找到包含给定时间的 tempo segment。
    fn find_segment_at(&self, time: f64) -> Option<&TempoSegment> {
        if self.tempo_segments.is_empty() {
            return None;
        }
        self.tempo_segments
            .iter()
            .rposition(|s| s.start_time <= time)
            .map(|idx| &self.tempo_segments[idx])
    }

    /// 将秒时间转换为对应的 MIDI tick（考虑 tempo 变化）。
    pub fn tick_at_time(&self, time: f64) -> f64 {
        let Some(seg) = self.find_segment_at(time) else {
            return seconds_to_ticks(time, self.ticks_per_beat, DEFAULT_MPQ);
        };
        let dt = time - seg.start_time;
        seg.start_tick as f64 + seconds_to_ticks(dt, self.ticks_per_beat, seg.micros_per_quarter)
    }

    /// 获取指定时间的 BPM。
    pub fn bpm_at_time(&self, time: f64) -> f32 {
        let Some(seg) = self.find_segment_at(time) else {
            return DEFAULT_BPM as f32;
        };
        bpm_from_mpq(seg.micros_per_quarter)
    }

    /// 计算每小节的 tick 数（bar divide）。
    pub fn bar_divide(&self) -> f64 {
        let num = self.time_sig_numerator as f64;
        let den = (1u32 << self.time_sig_denominator) as f64;
        self.ticks_per_beat as f64 * num / den * 4.0
    }

    /// 计算给定 tick 所在的小节号（1-based）。
    pub fn bar_at_tick(&self, tick: u64) -> u64 {
        crate::time::bar_at_tick(tick, self.bar_divide())
    }

    /// 计算总小节数。
    pub fn total_bars(&self) -> u64 {
        crate::time::total_bars(self.tick_length, self.bar_divide())
    }
}
