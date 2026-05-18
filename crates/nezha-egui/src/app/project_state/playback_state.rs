use std::time::Instant;

/// 播放时钟状态。
pub struct PlaybackState {
    pub is_playing: bool,
    pub current_time: f64,
    pub start: Option<(Instant, f64)>,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            is_playing: false,
            current_time: 0.0,
            start: None,
        }
    }
}

impl PlaybackState {
    pub fn reset(&mut self) {
        self.is_playing = false;
        self.current_time = 0.0;
        self.start = None;
    }
}
