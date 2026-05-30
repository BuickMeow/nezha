use eframe::egui;

use super::types::{Track, TrackClip, TrackKind, ClipKind};
use super::interaction::TimelineInteraction;

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
            tracks: vec![Track::new_video(&next_video_track_name(&[]))],
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
            ClipKind::Audio => {
                TrackClip::new_audio(id, format!("音频 {}", type_count), 0, 5.0)
            }
            ClipKind::Image => {
                TrackClip::new_image(id, format!("图片 {}", type_count), 0, 5.0)
            }
            ClipKind::Video => {
                TrackClip::new_video(id, format!("视频 {}", type_count), 0, 5.0)
            }
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
            let mut track = Track::new_video(&name_fn(&self.tracks));
            track.kind = target_kind;
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
        let mut clip = TrackClip::new_image(id, name, media_idx, duration);
        clip.end = duration;

        let target_kind = TrackKind::Video;
        if let Some(empty_track) = self
            .tracks
            .iter_mut()
            .find(|t| t.kind == target_kind && t.clips.is_empty())
        {
            empty_track.clips.push(clip);
        } else {
            let mut track = Track::new_video(&next_video_track_name(&self.tracks));
            track.clips.push(clip);
            self.tracks.insert(0, track);
        }
        id
    }

    pub fn push_video_clip(&mut self, media_idx: usize, name: String, duration: f32) -> usize {
        let id = self.alloc_clip_id();
        let mut clip = TrackClip::new_video(id, name, media_idx, duration);
        clip.end = duration;

        let target_kind = TrackKind::Video;
        if let Some(empty_track) = self
            .tracks
            .iter_mut()
            .find(|t| t.kind == target_kind && t.clips.is_empty())
        {
            empty_track.clips.push(clip);
        } else {
            let mut track = Track::new_video(&next_video_track_name(&self.tracks));
            track.clips.push(clip);
            self.tracks.insert(0, track);
        }
        id
    }

    pub fn push_audio_clip(&mut self, audio_idx: usize, name: String, duration: f32) -> usize {
        let id = self.alloc_clip_id();
        let clip = TrackClip::new_audio(id, name, audio_idx, duration);

        let target_kind = TrackKind::Audio;
        if let Some(empty_track) = self
            .tracks
            .iter_mut()
            .find(|t| t.kind == target_kind && t.clips.is_empty())
        {
            empty_track.clips.push(clip);
        } else {
            let mut track = Track::new_audio(&next_audio_track_name(&self.tracks));
            track.clips.push(clip);
            self.tracks.insert(0, track);
        }
        id
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
        interaction: &mut TimelineInteraction,
    ) {
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

        let track_already_inserted = interaction
            .clip_drag
            .map(|d| d.track_was_inserted)
            .unwrap_or(false);

        let dest_index = if target_track_index == 0 {
            if src_track_idx == Some(0) && !track_already_inserted {
                let video_count = self
                    .tracks
                    .iter()
                    .filter(|t| t.kind == TrackKind::Video)
                    .count();
                let name = format!("视频 {}", video_count + 1);
                self.tracks.insert(0, Track::new_video(&name));
                if let Some(ref mut drag) = interaction.clip_drag {
                    drag.track_was_inserted = true;
                }
                Some(0)
            } else if self.tracks[0].kind == TrackKind::Video {
                Some(0)
            } else {
                self.tracks.iter().position(|t| t.kind == TrackKind::Video)
            }
        } else if target_track_index < self.tracks.len() {
            if self.tracks[target_track_index].kind == TrackKind::Video {
                Some(target_track_index)
            } else {
                self.tracks.iter().position(|t| t.kind == TrackKind::Video)
            }
        } else {
            src_track_idx
        };

        if let Some(idx) = dest_index {
            self.tracks[idx].clips.push(clip);
        } else if src_track_idx.is_none() {
            let name = format!(
                "视频 {}",
                self.tracks
                    .iter()
                    .filter(|t| t.kind == TrackKind::Video)
                    .count()
                    + 1
            );
            let mut track = Track::new_video(&name);
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
}

pub fn snap_to_frame(time: f32, frame_duration: f32) -> f32 {
    if frame_duration <= 0.0 {
        return time.max(0.0);
    }
    (time / frame_duration).round() * frame_duration
}
