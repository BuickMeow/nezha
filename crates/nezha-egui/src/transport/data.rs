use eframe::egui;

use super::interaction::TimelineInteraction;
use super::types::{ClipKind, Track, TrackClip, TrackKind};

pub fn next_video_track_name(tracks: &[Track]) -> String {
    let count = tracks.iter().filter(|t| t.kind == TrackKind::Video).count();
    format!("视频 {}", count + 1)
}

pub fn next_audio_track_name(tracks: &[Track]) -> String {
    let count = tracks.iter().filter(|t| t.kind == TrackKind::Audio).count();
    format!("音频 {}", count + 1)
}

#[derive(Clone, Debug)]
pub struct TimelineData {
    pub tracks: Vec<Track>,
    next_clip_id: usize,
}

impl Default for TimelineData {
    fn default() -> Self {
        Self {
            tracks: vec![Track::new(&next_video_track_name(&[]), TrackKind::Video)],
            next_clip_id: 1,
        }
    }
}

impl TimelineData {
    pub fn alloc_clip_id(&mut self) -> usize {
        let id = self.next_clip_id;
        self.next_clip_id += 1;
        id
    }

    fn recompute_next_clip_id(&mut self) {
        let max_id = self
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .map(|c| c.id)
            .max()
            .unwrap_or(0);
        self.next_clip_id = max_id + 1;
    }

    pub fn push_clip(
        &mut self,
        kind: ClipKind,
        duration: f32,
        midi_idx: Option<usize>,
        color: egui::Color32,
        content_start_offset: u32,
        content_end_offset: u32,
    ) -> usize {
        let id = self.alloc_clip_id();

        let type_count = self
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .filter(|c| c.kind == kind)
            .count()
            + 1;
        let mut clip = match kind {
            ClipKind::Waterfall => {
                let mut c = TrackClip::new_waterfall(id, midi_idx);
                c.name = format!("默认瀑布流 {}", type_count);
                c.content_start_offset = content_start_offset;
                c.content_end_offset = content_end_offset;
                c
            }
            ClipKind::SolidColor => {
                let mut c = TrackClip::new_solid_color(id, color);
                c.name = format!("纯色 {}", type_count);
                c
            }
            ClipKind::Counter => {
                let mut c = TrackClip::new_counter(id, midi_idx);
                c.name = format!("计数器 {}", type_count);
                c
            }
            ClipKind::Audio => TrackClip::new_audio(id, format!("音频 {}", type_count), 0, 5.0),
            ClipKind::Image => TrackClip::new_image(id, format!("图片 {}", type_count), 0, 5.0),
            ClipKind::Video => TrackClip::new_video(id, format!("视频 {}", type_count), 0, 5.0),
        };
        clip.end = if duration > 0.0 { duration } else { 5.0 };

        let target_kind = match kind {
            ClipKind::Audio => TrackKind::Audio,
            _ => TrackKind::Video,
        };
        let name_fn = match kind {
            ClipKind::Audio => next_audio_track_name as fn(&[Track]) -> String,
            _ => next_video_track_name as fn(&[Track]) -> String,
        };

        if let Some(empty_track) = self
            .tracks
            .iter_mut()
            .find(|t| t.kind == target_kind && t.clips.is_empty())
        {
            empty_track.clips.push(clip);
        } else {
            let mut track = Track::new(&name_fn(&self.tracks), target_kind);
            track.clips.push(clip);
            self.tracks.insert(0, track);
        }
        id
    }

    pub fn push_waterfall_clip(
        &mut self,
        midi_idx: Option<usize>,
        duration: f32,
        song_start_time: f32,
        song_duration: f32,
    ) -> usize {
        let id = self.push_clip(
            ClipKind::Waterfall,
            duration,
            midi_idx,
            egui::Color32::TRANSPARENT,
            0,
            0,
        );
        if let Some(clip) = self.find_clip_mut(id) {
            clip.song_start_time = song_start_time;
            clip.song_duration = song_duration;
        }
        id
    }

    pub fn push_solid_color_clip(&mut self, color: egui::Color32, duration: f32) -> usize {
        self.push_clip(ClipKind::SolidColor, duration, None, color, 0, 0)
    }

    pub fn push_counter_clip(&mut self, duration: f32, midi_idx: Option<usize>) -> usize {
        self.push_clip(
            ClipKind::Counter,
            duration,
            midi_idx,
            egui::Color32::TRANSPARENT,
            0,
            0,
        )
    }

    pub fn push_image_clip(&mut self, media_idx: usize, name: String, duration: f32) -> usize {
        let id = self.alloc_clip_id();
        let clip = TrackClip::new_image(id, name, media_idx, duration);
        self.insert_clip_into_track(clip, TrackKind::Video);
        id
    }

    pub fn push_video_clip(&mut self, media_idx: usize, name: String, duration: f32) -> usize {
        let id = self.alloc_clip_id();
        let clip = TrackClip::new_video(id, name, media_idx, duration);
        self.insert_clip_into_track(clip, TrackKind::Video);
        id
    }

    pub fn push_audio_clip(&mut self, audio_idx: usize, name: String, duration: f32) -> usize {
        let id = self.alloc_clip_id();
        let clip = TrackClip::new_audio(id, name, audio_idx, duration);
        self.insert_clip_into_track(clip, TrackKind::Audio);
        id
    }

    /// Insert a clip into the first empty track of the given kind, or create a new track.
    fn insert_clip_into_track(&mut self, clip: TrackClip, target_kind: TrackKind) {
        if let Some(empty_track) = self
            .tracks
            .iter_mut()
            .find(|t| t.kind == target_kind && t.clips.is_empty())
        {
            empty_track.clips.push(clip);
        } else {
            let track_name = if target_kind == TrackKind::Video {
                next_video_track_name(&self.tracks)
            } else {
                next_audio_track_name(&self.tracks)
            };
            let mut track = Track::new(&track_name, target_kind);
            track.clips.push(clip);
            self.tracks.insert(0, track);
        }
    }

    pub fn remove_clip(&mut self, clip_id: usize) -> bool {
        let mut found = false;
        for track in &mut self.tracks {
            let before = track.clips.len();
            track.clips.retain(|clip| clip.id != clip_id);
            if track.clips.len() < before {
                found = true;
            }
        }
        self.recompute_next_clip_id();
        found
    }

    pub fn content_duration(&self) -> f32 {
        self.tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .map(|c| c.end)
            .fold(0.0, f32::max)
    }

    pub fn update_duration(&mut self, duration: f32) {
        for track in &mut self.tracks {
            for clip in &mut track.clips {
                if clip.end == 0.0 {
                    clip.end = duration;
                }
            }
        }
    }

    pub fn move_clip_to_start(&mut self, clip_id: usize, new_start: f32, fps: u32) {
        let frame_duration = Self::frame_duration(fps);
        if let Some(clip) = self.find_clip_mut(clip_id) {
            let old_start = clip.start;
            let width = clip.end - clip.start;
            clip.start = snap_to_frame(new_start.max(0.0), frame_duration);
            clip.end = clip.start + width;
            clip.song_start_time += clip.start - old_start;
            clip.update_content_offsets(fps);
        }
    }

    pub fn resize_clip_start_to(&mut self, clip_id: usize, new_start: f32, fps: u32) {
        let frame_duration = Self::frame_duration(fps);
        if let Some(clip) = self.find_clip_mut(clip_id) {
            clip.start = snap_to_frame(new_start.max(0.0), frame_duration);
            clip.start = clip.start.min(clip.end - frame_duration);
            clip.update_content_offsets(fps);
        }
    }

    pub fn resize_clip_end_to(&mut self, clip_id: usize, new_end: f32, fps: u32) {
        let frame_duration = Self::frame_duration(fps);
        if let Some(clip) = self.find_clip_mut(clip_id) {
            clip.end = snap_to_frame(new_end.max(clip.start + frame_duration), frame_duration);
            clip.update_content_offsets(fps);
        }
    }

    pub fn move_clip_to_track(
        &mut self,
        clip_id: usize,
        target_track_index: usize,
        target_track_kind: TrackKind,
        interaction: &mut TimelineInteraction,
    ) {
        // 1. 找到并移除 Clip
        let mut src_track_idx = None;
        let mut clip_to_move = None;
        'outer: for (i, track) in self.tracks.iter_mut().enumerate() {
            for (j, clip) in track.clips.iter().enumerate() {
                if clip.id == clip_id {
                    clip_to_move = Some(track.clips.remove(j));
                    src_track_idx = Some(i);
                    break 'outer;
                }
            }
        }

        let Some(clip) = clip_to_move else {
            return;
        };

        // 2. 检查是否已经创建过新轨道
        let track_already_inserted = interaction
            .clip_drag
            .map(|d| d.track_was_inserted)
            .unwrap_or(false);

        // 3. 根据目标轨道类型决定名称前缀
        let name_prefix = match target_track_kind {
            TrackKind::Video => "视频",
            TrackKind::Audio => "音频",
        };

        // 4. 计算目标位置
        // 先找到同类轨道的全局索引列表
        let same_kind_indices: Vec<usize> = self
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.kind == target_track_kind)
            .map(|(i, _)| i)
            .collect();

        let dest_index = if target_track_index < same_kind_indices.len() {
            // 目标在同类轨道范围内
            Some(same_kind_indices[target_track_index])
        } else if !track_already_inserted {
            // 目标超出范围，创建新轨道（如果还没创建过）
            let count = same_kind_indices.len();
            let name = format!("{} {}", name_prefix, count + 1);
            let new_track = Track::new(&name, target_track_kind);
            // 新轨道插入到同类轨道组的末尾
            let insert_pos = same_kind_indices.last().map(|&i| i + 1).unwrap_or(0);
            self.tracks.insert(insert_pos, new_track);
            if let Some(ref mut drag) = interaction.clip_drag {
                drag.track_was_inserted = true;
            }
            Some(insert_pos)
        } else {
            // 已经创建过新轨道，放回原轨道
            src_track_idx
        };

        // 5. 放置 Clip 到目标轨道
        if let Some(idx) = dest_index {
            if self.tracks[idx].locked {
                // 目标轨道被锁定，放回原轨道
                if let Some(src_idx) = src_track_idx {
                    self.tracks[src_idx].clips.push(clip);
                }
                return;
            }
            self.tracks[idx].clips.push(clip);
        } else {
            // 找不到目标轨道，创建新的
            let count = same_kind_indices.len();
            let name = format!("{} {}", name_prefix, count + 1);
            let mut track = Track::new(&name, target_track_kind);
            track.clips.push(clip);
            self.tracks.push(track);
        }
    }

    fn frame_duration(fps: u32) -> f32 {
        1.0 / fps.max(1) as f32
    }

    pub fn find_clip_mut(&mut self, clip_id: usize) -> Option<&mut TrackClip> {
        for track in &mut self.tracks {
            if let Some(clip) = track.clips.iter_mut().find(|clip| clip.id == clip_id) {
                return Some(clip);
            }
        }
        None
    }

    pub fn toggle_track_mute(&mut self, track_index: usize) {
        if let Some(track) = self.tracks.get_mut(track_index) {
            track.muted = !track.muted;
        }
    }

    pub fn toggle_track_hidden(&mut self, track_index: usize) {
        if let Some(track) = self.tracks.get_mut(track_index) {
            track.hidden = !track.hidden;
        }
    }

    pub fn toggle_track_locked(&mut self, track_index: usize) {
        if let Some(track) = self.tracks.get_mut(track_index) {
            track.locked = !track.locked;
        }
    }
}

pub fn snap_to_frame(time: f32, frame_duration: f32) -> f32 {
    if frame_duration <= 0.0 {
        return time.max(0.0);
    }
    (time / frame_duration).round() * frame_duration
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snap_to_frame_basic() {
        let fd = 1.0 / 60.0;
        let snapped = snap_to_frame(0.5, fd);
        assert!((snapped * 60.0).round() - snapped * 60.0 < 0.01);
    }

    #[test]
    fn test_snap_to_frame_zero() {
        assert_eq!(snap_to_frame(0.0, 1.0 / 60.0), 0.0);
    }

    #[test]
    fn test_snap_to_frame_negative() {
        let fd = 1.0 / 60.0;
        let snapped = snap_to_frame(-1.0, fd);
        assert!(snapped <= 0.0);
    }

    #[test]
    fn test_snap_to_frame_zero_duration() {
        assert_eq!(snap_to_frame(5.0, 0.0), 5.0);
    }

    #[test]
    fn test_snap_to_frame_exact() {
        let fd = 1.0 / 30.0;
        let snapped = snap_to_frame(1.0 / 30.0, fd);
        assert!((snapped - fd).abs() < 0.001);
    }

    #[test]
    fn test_timeline_data_push_and_remove() {
        let mut data = TimelineData::default();
        let id1 = data.push_solid_color_clip(egui::Color32::RED, 5.0);
        let id2 = data.push_solid_color_clip(egui::Color32::BLUE, 10.0);
        assert!(id1 != id2);

        let total: usize = data.tracks.iter().map(|t| t.clips.len()).sum();
        assert_eq!(total, 2);

        assert!(data.remove_clip(id1));
        let total: usize = data.tracks.iter().map(|t| t.clips.len()).sum();
        assert_eq!(total, 1);

        assert!(!data.remove_clip(999));
    }

    #[test]
    fn test_timeline_data_content_duration() {
        let mut data = TimelineData::default();
        data.push_solid_color_clip(egui::Color32::RED, 5.0);
        data.push_solid_color_clip(egui::Color32::BLUE, 10.0);
        assert!((data.content_duration() - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_timeline_data_update_duration() {
        let mut data = TimelineData::default();
        let id = data.alloc_clip_id();
        let mut clip = TrackClip::new_solid_color(id, egui::Color32::RED);
        clip.end = 0.0;
        data.tracks[0].clips.push(clip);
        data.update_duration(30.0);
        let clip = data.find_clip_mut(id).unwrap();
        assert!((clip.end - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_timeline_data_move_clip_to_start() {
        let mut data = TimelineData::default();
        data.push_solid_color_clip(egui::Color32::RED, 10.0);
        data.move_clip_to_start(1, 5.0, 60);
        let clip = data.find_clip_mut(1).unwrap();
        assert!((clip.start - 5.0).abs() < 0.02);
        assert!((clip.end - 15.0).abs() < 0.02);
    }

    #[test]
    fn test_timeline_data_resize_clip_end() {
        let mut data = TimelineData::default();
        data.push_solid_color_clip(egui::Color32::RED, 10.0);
        data.resize_clip_end_to(1, 20.0, 60);
        let clip = data.find_clip_mut(1).unwrap();
        assert!((clip.end - 20.0).abs() < 0.02);
    }

    #[test]
    fn test_timeline_data_resize_clip_start() {
        let mut data = TimelineData::default();
        data.push_solid_color_clip(egui::Color32::RED, 10.0);
        data.resize_clip_start_to(1, 3.0, 60);
        let clip = data.find_clip_mut(1).unwrap();
        assert!((clip.start - 3.0).abs() < 0.02);
    }

    #[test]
    fn test_next_track_names() {
        let tracks = vec![];
        assert_eq!(next_video_track_name(&tracks), "视频 1");
        assert_eq!(next_audio_track_name(&tracks), "音频 1");

        let mut data = TimelineData::default();
        data.push_solid_color_clip(egui::Color32::RED, 1.0);
        assert_eq!(next_video_track_name(&data.tracks), "视频 2");
    }

    #[test]
    fn test_alloc_clip_id_monotonic() {
        let mut data = TimelineData::default();
        let id1 = data.alloc_clip_id();
        let id2 = data.alloc_clip_id();
        let id3 = data.alloc_clip_id();
        assert!(id1 < id2);
        assert!(id2 < id3);
    }

    #[test]
    fn test_push_image_video_audio_clips() {
        let mut data = TimelineData::default();
        let img_id = data.push_image_clip(0, "test.png".into(), 5.0);
        let vid_id = data.push_video_clip(0, "test.mp4".into(), 10.0);
        let aud_id = data.push_audio_clip(0, "test.mp3".into(), 3.0);

        let img = data.find_clip_mut(img_id).unwrap();
        assert_eq!(img.kind, ClipKind::Image);
        assert_eq!(img.media_idx, Some(0));

        let vid = data.find_clip_mut(vid_id).unwrap();
        assert_eq!(vid.kind, ClipKind::Video);

        let aud = data.find_clip_mut(aud_id).unwrap();
        assert_eq!(aud.kind, ClipKind::Audio);
        assert_eq!(aud.audio_idx, Some(0));
    }
}
