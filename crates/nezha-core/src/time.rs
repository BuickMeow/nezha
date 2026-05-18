/// 默认 microseconds per quarter note，对应 120 BPM。
pub const DEFAULT_MPQ: u64 = 500_000;

/// 默认 BPM。
pub const DEFAULT_BPM: f64 = 120.0;

/// 每秒微秒数。
pub const MICROS_PER_SEC: f64 = 1_000_000.0;

/// 每分钟微秒数。
pub const MICROS_PER_MINUTE: f64 = 60_000_000.0;

/// Timecode 模式下的 fallback ticks per beat。
pub const TIMECODE_FALLBACK_TPB: u32 = 480;

/// 一段连续的 tempo 区间。
#[derive(Clone, Debug)]
pub struct TempoSegment {
    pub start_tick: u32,
    pub start_time: f64,
    pub micros_per_quarter: u64,
}

/// 判断 MIDI key 是否为黑键。
pub const fn is_black_key(key: u8) -> bool {
    matches!(key % 12, 1 | 3 | 6 | 8 | 10)
}

/// 将 tick 差值转换为秒数。
pub fn ticks_to_seconds(dtick: u32, ticks_per_beat: u32, micros_per_quarter: u64) -> f64 {
    (dtick as u64 * micros_per_quarter) as f64 / (ticks_per_beat as f64 * MICROS_PER_SEC)
}

/// 将秒数差值转换为 tick。
pub fn seconds_to_ticks(dt: f64, ticks_per_beat: u32, micros_per_quarter: u64) -> f64 {
    dt * ticks_per_beat as f64 * MICROS_PER_SEC / micros_per_quarter as f64
}

/// 从 microseconds per quarter 计算 BPM。
pub fn bpm_from_mpq(mpq: u64) -> f32 {
    (MICROS_PER_MINUTE / mpq as f64) as f32
}
