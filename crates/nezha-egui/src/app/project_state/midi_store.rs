use crate::transport::{ClipKind, TimelineState, Track, TrackClip, TrackKind};
use nezha_core::MidiFile;

/// 一个已加载的 MIDI 条目。
#[derive(Clone, Debug)]
pub struct MidiEntry {
    pub path: String,
    pub file: MidiFile,
}

/// 项目中的 MIDI 资源集合与当前高亮选择。
#[derive(Default)]
pub struct MidiStore {
    pub entries: Vec<MidiEntry>,
    pub highlighted_idx: Option<usize>,
}

impl MidiStore {
    /// 最大同时加载的 MIDI 文件数量。
    const MAX_MIDI_FILES: usize = 16;

    pub fn highlighted_midi(&self) -> Option<&MidiFile> {
        self.highlighted_idx
            .and_then(|idx| self.entries.get(idx))
            .map(|entry| &entry.file)
    }

    pub fn insert(
        &mut self,
        path: String,
        midi: MidiFile,
        timeline_state: &mut TimelineState,
    ) -> usize {
        if self.entries.len() >= Self::MAX_MIDI_FILES {
            self.entries.remove(0);
            self.adjust_highlight_after_removal(0);
            self.remap_clip_indices_after_removal(0, timeline_state);
        }

        let idx = self.entries.len();
        self.entries.push(MidiEntry { path, file: midi });
        self.highlighted_idx = Some(idx);
        self.bind_unassigned_waterfalls(idx, timeline_state);
        idx
    }

    pub fn remove(&mut self, idx: usize, timeline_state: &mut TimelineState) {
        if idx >= self.entries.len() {
            return;
        }

        self.entries.remove(idx);
        self.adjust_highlight_after_removal(idx);
        self.remap_clip_indices_after_removal(idx, timeline_state);
    }

    fn adjust_highlight_after_removal(&mut self, removed_idx: usize) {
        self.highlighted_idx = match self.highlighted_idx {
            Some(idx) if idx == removed_idx => {
                if removed_idx > 0 {
                    Some(removed_idx - 1)
                } else {
                    self.entries.first().map(|_| 0)
                }
            }
            Some(idx) if idx > removed_idx => Some(idx - 1),
            other => other,
        };
    }

    fn remap_clip_indices_after_removal(
        &mut self,
        removed_idx: usize,
        timeline_state: &mut TimelineState,
    ) {
        for track in &mut timeline_state.data.tracks {
            for clip in &mut track.clips {
                match clip.midi_idx {
                    Some(idx) if idx == removed_idx => clip.midi_idx = None,
                    Some(idx) if idx > removed_idx => clip.midi_idx = Some(idx - 1),
                    _ => {}
                }
            }
        }
    }

    /// 根据 MIDI 音符数据计算 content_start_offset 和 content_end_offset（帧数）。
    #[allow(dead_code)]
    pub fn calculate_content_offsets(midi: &MidiFile, fps: u32) -> (u32, u32) {
        let fps_f64 = fps.max(1) as f64;

        let mut first_note_time = f64::MAX;
        let mut last_note_end = 0.0f64;

        for notes in &midi.key_notes {
            for note in notes {
                if note.start < first_note_time {
                    first_note_time = note.start;
                }
                if note.end > last_note_end {
                    last_note_end = note.end;
                }
            }
        }

        let start_offset = if first_note_time == f64::MAX {
            0
        } else {
            (first_note_time * fps_f64).round() as u32
        };

        let end_offset = if last_note_end <= 0.0 {
            0
        } else {
            ((midi.duration - last_note_end) * fps_f64).round().max(0.0) as u32
        };

        (start_offset, end_offset)
    }

    /// 默认前奏缓冲区（秒），在歌曲开始之前的深蓝色区域。
    pub const DEFAULT_PRE_SONG_BUFFER: f32 = 0.0;

    fn bind_unassigned_waterfalls(&mut self, midi_idx: usize, timeline_state: &mut TimelineState) {
        let midi_duration = self
            .entries
            .get(midi_idx)
            .map(|e| e.file.duration as f32)
            .unwrap_or(5.0);
        let pre_song = Self::DEFAULT_PRE_SONG_BUFFER;

        // 先尝试绑定已有的未分配 MIDI 的瀑布流 clip
        for track in &mut timeline_state.data.tracks {
            for clip in &mut track.clips {
                if clip.kind == ClipKind::Waterfall && clip.midi_idx.is_none() {
                    clip.midi_idx = Some(midi_idx);
                    clip.song_start_time = clip.start + pre_song;
                    clip.song_duration = midi_duration;
                    clip.end = clip.song_start_time + midi_duration;
                    clip.update_content_offsets(timeline_state.fps);
                    return;
                }
            }
        }

        // 没有未绑定的 clip → 在首个视频轨道上新建一个
        let id = timeline_state.data.alloc_clip_id();
        let mut clip = TrackClip::new_waterfall(id, Some(midi_idx));
        clip.song_start_time = pre_song;
        clip.song_duration = midi_duration;
        clip.end = pre_song + midi_duration;
        clip.update_content_offsets(timeline_state.fps);

        if let Some(track) = timeline_state
            .data
            .tracks
            .iter_mut()
            .find(|t| t.kind == TrackKind::Video)
        {
            track.clips.push(clip);
        } else {
            let name = crate::transport::next_video_track_name(&timeline_state.data.tracks);
            let mut track = Track::new_video(&name);
            track.clips.push(clip);
            timeline_state.data.tracks.push(track);
        }
    }
}
