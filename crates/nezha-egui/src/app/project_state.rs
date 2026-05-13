use crate::transport::TimelineState;
use nezha_core::MidiFile;
use std::time::Instant;

/// 一个已加载的 MIDI 条目
#[derive(Clone, Debug)]
pub struct MidiEntry {
    pub path: String,
    pub file: MidiFile,
}

const MAX_MIDI_FILES: usize = 16;

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
    /// 最近一次错误信息（用于 UI 提示）
    pub last_error: Option<String>,
}

impl ProjectState {
    pub fn new() -> Self {
        let mut timeline_state = TimelineState::default();
        timeline_state.fps = 60;
        Self {
            is_playing: false,
            current_time: 0.0,
            midi_files: Vec::new(),
            last_error: None,
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

    /// 通过索引获取 MIDI 条目
    pub fn midi_entry(&self, idx: usize) -> Option<&MidiEntry> {
        self.midi_files.get(idx)
    }

    /// 当前总时长（由高亮 MIDI 决定，如果没有则默认 120s）
    pub fn duration(&self) -> f64 {
        self.highlighted_midi().map(|m| m.duration).unwrap_or(120.0)
    }

    /// 返回 Ok(idx) 表示加载成功及新 MIDI 的索引
    pub fn load_midi(&mut self, path: String) -> Result<usize, String> {
        match MidiFile::load(&path) {
            Ok(midi) => {
                // 限制最大 MIDI 文件数，防止内存无限增长
                if self.midi_files.len() >= MAX_MIDI_FILES {
                    self.midi_files.remove(0);
                    // 更新高亮索引和 clip 引用
                    if let Some(ref mut h) = self.highlighted_midi_idx {
                        if *h == 0 {
                            *h = self.midi_files.len().saturating_sub(1);
                        } else {
                            *h -= 1;
                        }
                    }
                    for track in &mut self.timeline_state.data.tracks {
                        for clip in &mut track.clips {
                            match clip.midi_idx {
                                Some(0) => clip.midi_idx = None,
                                Some(i) => clip.midi_idx = Some(i - 1),
                                _ => {}
                            }
                        }
                    }
                }
                let idx = self.midi_files.len();
                self.midi_files.push(MidiEntry { path, file: midi });
                self.highlighted_midi_idx = Some(idx);
                // 把所有未绑定的瀑布流 clip 都绑定到这个新 MIDI
                for track in &mut self.timeline_state.data.tracks {
                    for clip in &mut track.clips {
                        if clip.kind == crate::transport::ClipKind::Waterfall
                            && clip.midi_idx.is_none()
                        {
                            clip.midi_idx = Some(idx);
                        }
                    }
                }
                let dur = self.duration();
                self.timeline_state.update_duration(dur as f32);
                self.current_time = 0.0;
                self.playback_start = None;
                let idx = self.midi_files.len() - 1;
                Ok(idx)
            }
            Err(e) => {
                let msg = format!("MIDI 加载失败: {}", e);
                eprintln!("{}", msg);
                self.last_error = Some(msg.clone());
                Err(msg)
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
                if idx > 0 {
                    Some(idx - 1)
                } else {
                    self.midi_files.first().map(|_| 0)
                }
            }
            Some(h) if h > idx => Some(h - 1),
            other => other,
        };
        // 更新所有 clip 中引用的 midi_idx
        for track in &mut self.timeline_state.data.tracks {
            for clip in &mut track.clips {
                match clip.midi_idx {
                    Some(i) if i == idx => clip.midi_idx = None,
                    Some(i) if i > idx => clip.midi_idx = Some(i - 1),
                    _ => {}
                }
            }
        }
        let dur = self.duration();
        self.timeline_state.update_duration(dur as f32);
    }

    pub fn clear_all_midi(&mut self) {
        self.midi_files.clear();
        self.highlighted_midi_idx = None;
        for track in &mut self.timeline_state.data.tracks {
            for clip in &mut track.clips {
                clip.midi_idx = None;
            }
        }
    }
}
