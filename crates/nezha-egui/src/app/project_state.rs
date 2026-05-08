use std::time::Instant;
use nezha_core::MidiFile;
use crate::app::RenderContext;
use crate::transport::TimelineState;

/// 一个已加载的 MIDI 条目
#[derive(Clone, Debug)]
pub struct MidiEntry {
    pub path: String,
    pub file: MidiFile,
}

pub struct ProjectState {
    pub is_playing: bool,
    pub current_time: f64,
    pub midi_files: Vec<MidiEntry>,
    /// 当前高亮的 MIDI 索引（决定渲染和添加瀑布流用哪个）
    pub highlighted_midi_idx: Option<usize>,
    pub render_width: u32,
    pub render_height: u32,
    pub fps: u32,
    pub timeline_state: TimelineState,
    pub playback_start: Option<(Instant, f64)>,
}

impl ProjectState {
    pub fn new() -> Self {
        let mut timeline_state = TimelineState::default();
        timeline_state.fps = 60;
        Self {
            is_playing: false,
            current_time: 0.0,
            midi_files: Vec::new(),
            highlighted_midi_idx: None,
            render_width: 1920,
            render_height: 1080,
            fps: 60,
            timeline_state,
            playback_start: None,
        }
    }

    /// 当前高亮的 MIDI 文件，用于渲染
    pub fn highlighted_midi(&self) -> Option<&MidiFile> {
        self.highlighted_midi_idx
            .and_then(|idx| self.midi_files.get(idx))
            .map(|e| &e.file)
    }

    /// 当前高亮 MIDI 的路径
    pub fn highlighted_midi_path(&self) -> Option<&str> {
        self.highlighted_midi_idx
            .and_then(|idx| self.midi_files.get(idx))
            .map(|e| e.path.as_str())
    }

    /// 当前总时长（由高亮 MIDI 决定，如果没有则默认 120s）
    pub fn duration(&self) -> f64 {
        self.highlighted_midi()
            .map(|m| m.duration)
            .unwrap_or(120.0)
    }

    pub fn load_midi(&mut self, path: String, render_ctx: &mut RenderContext) {
        match MidiFile::load(&path) {
            Ok(midi) => {
                let idx = self.midi_files.len();
                self.midi_files.push(MidiEntry { path, file: midi });
                self.highlighted_midi_idx = Some(idx);
                let dur = self.duration();
                self.timeline_state.update_duration(dur as f32);
                self.current_time = 0.0;
                self.playback_start = None;
                render_ctx.reset_midi_state();
            }
            Err(e) => {
                eprintln!("Failed to load MIDI: {}", e);
            }
        }
    }

    pub fn remove_midi(&mut self, idx: usize) {
        if idx >= self.midi_files.len() {
            return;
        }
        self.midi_files.remove(idx);
        // 调整高亮索引
        self.highlighted_midi_idx = match self.highlighted_midi_idx {
            Some(h) if h == idx => {
                // 删掉了高亮的，选前一个或保持 None
                if idx > 0 { Some(idx - 1) } else { self.midi_files.first().map(|_| 0) }
            }
            Some(h) if h > idx => Some(h - 1),
            other => other,
        };
        let dur = self.duration();
        self.timeline_state.update_duration(dur as f32);
    }
}
