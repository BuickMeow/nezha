// ── Renderer performance tuning constants ────────────────────────────────

/// Base pixels-per-second for time-based scroll mode.
pub const PIXELS_PER_SEC_BASE: f64 = 200.0;

/// Fallback ticks per beat when MIDI file uses timecode timing.
pub const TIMECODE_FALLBACK_TPB: u32 = 480;

/// Minimum speed multiplier (clamped to avoid division by zero).
pub const MIN_SPEED: f32 = 0.01;

/// Maximum number of note instances per draw batch.
pub const MAX_INSTANCE_COUNT: usize = 6_000_000;

/// Maximum number of parallel key groups for rayon work distribution.
pub const MAX_PARALLEL_KEY_GROUPS: usize = 16;

/// Block size for the seek index prefix array.
pub const SEEK_INDEX_BLOCK_SIZE: usize = 256;

/// Minimum instance buffer capacity (in instances).
pub const MIN_INSTANCE_BUFFER_CAPACITY: usize = 4_096;

/// Puffin profiling server port.
#[cfg(feature = "profiling")]
pub const PUFFIN_PORT: u16 = 8585;


