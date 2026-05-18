use crate::transport::{ClipKind, TimelineState};
use nezha_core::MidiFile;

/// 一个已加载的 MIDI 条目。
#[derive(Clone, Debug)]
pub struct MidiEntry {
    pub path: String,
    pub file: MidiFile,
}

/// 项目中的 MIDI 资源集合与当前高亮选择。
pub struct MidiStore {
    pub entries: Vec<MidiEntry>,
    pub highlighted_idx: Option<usize>,
}

impl Default for MidiStore {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            highlighted_idx: None,
        }
    }
}

impl MidiStore {
    /// 最大同时加载的 MIDI 文件数量。
    const MAX_MIDI_FILES: usize = 16;

    pub fn highlighted_midi(&self) -> Option<&MidiFile> {
        self.highlighted_idx
            .and_then(|idx| self.entries.get(idx))
            .map(|entry| &entry.file)
    }

    pub fn insert(&mut self, path: String, midi: MidiFile, timeline_state: &mut TimelineState) -> usize {
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

    fn remap_clip_indices_after_removal(&mut self, removed_idx: usize, timeline_state: &mut TimelineState) {
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

    fn bind_unassigned_waterfalls(&mut self, midi_idx: usize, timeline_state: &mut TimelineState) {
        for track in &mut timeline_state.data.tracks {
            for clip in &mut track.clips {
                if clip.kind == ClipKind::Waterfall && clip.midi_idx.is_none() {
                    clip.midi_idx = Some(midi_idx);
                }
            }
        }
    }
}
