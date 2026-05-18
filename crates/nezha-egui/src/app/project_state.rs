mod midi_store;
mod playback_state;
mod render_settings;

use crate::transport::TimelineState;
use nezha_core::MidiFile;

pub use midi_store::{MidiEntry, MidiStore};
pub use playback_state::PlaybackState;
pub use render_settings::RenderSettings;

/// 无 MIDI 时的默认时长（秒）。
const DEFAULT_DURATION_SECS: f64 = 120.0;

pub struct ProjectState {
    pub playback: PlaybackState,
    pub midi: MidiStore,
    pub render: RenderSettings,
    pub timeline_state: TimelineState,
    /// 最近一次错误信息（用于 UI 提示）
    pub last_error: Option<String>,
}

impl ProjectState {
    pub fn new() -> Self {
        let mut timeline_state = TimelineState::default();
        timeline_state.fps = 60;
        Self {
            playback: PlaybackState::default(),
            midi: MidiStore::default(),
            render: RenderSettings::default(),
            last_error: None,
            timeline_state,
        }
    }

    /// 当前高亮的 MIDI 文件，用于渲染
    pub fn highlighted_midi(&self) -> Option<&MidiFile> {
        self.midi.highlighted_midi()
    }

    /// 当前总时长。
    ///
    /// 取时间线中所有 clip 的最晚结束时间（有内容的最后一帧）。
    /// 如果时间线尚无内容（所有 clip 的 end 均为 0），则返回 0。
    pub fn duration(&self) -> f64 {
        self.timeline_state.content_duration() as f64
    }

    /// 将已解析的 MidiFile 插入项目，执行所有后处理逻辑
    pub fn insert_midi(&mut self, path: String, midi: MidiFile) -> usize {
        let duration = midi.duration;
        let idx = self.midi.insert(path, midi, &mut self.timeline_state);
        self.sync_timeline_settings();
        self.timeline_state.update_duration(duration as f32);
        self.playback.reset();
        idx
    }

    pub fn remove_midi(&mut self, idx: usize) {
        self.midi.remove(idx, &mut self.timeline_state);
        self.sync_timeline_settings();
        let fallback_duration = self
            .highlighted_midi()
            .map(|m| m.duration)
            .unwrap_or(DEFAULT_DURATION_SECS);
        self.timeline_state
            .update_duration(fallback_duration as f32);
        self.playback.current_time = self.playback.current_time.min(self.duration());
        self.playback.start = None;
    }

    fn sync_timeline_settings(&mut self) {
        self.timeline_state.fps = self.render.fps;
    }
}
