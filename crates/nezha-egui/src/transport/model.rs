use eframe::egui;

use super::data::TimelineData;
use super::interaction::TimelineInteraction;
use super::selection::TimelineSelection;
use super::view::TimelineView;

#[derive(Clone, Debug)]
pub struct TimelineState {
    pub view: TimelineView,
    pub data: TimelineData,
    pub interaction: TimelineInteraction,
    pub selection: TimelineSelection,
    pub fps: u32,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            view: TimelineView::default(),
            data: TimelineData::default(),
            interaction: TimelineInteraction::default(),
            selection: TimelineSelection::default(),
            fps: 60,
        }
    }
}

impl TimelineState {
    pub fn push_waterfall_clip(
        &mut self,
        midi_idx: Option<usize>,
        duration: f32,
        song_start_time: f32,
        song_duration: f32,
    ) {
        let fps = self.fps;
        let id = self.data.push_waterfall_clip(midi_idx, duration, song_start_time, song_duration);
        if let Some(clip) = self.data.find_clip_mut(id) {
            clip.update_content_offsets(fps);
        }
        self.selection.select(id);
    }

    pub fn push_solid_color_clip(&mut self, color: egui::Color32, duration: f32) {
        let id = self.data.push_solid_color_clip(color, duration);
        self.selection.select(id);
    }

    pub fn push_counter_clip(&mut self, duration: f32, midi_idx: Option<usize>) {
        let id = self.data.push_counter_clip(duration, midi_idx);
        self.selection.select(id);
    }

    pub fn push_image_clip(&mut self, media_idx: usize, name: String, duration: f32) {
        let id = self.data.push_image_clip(media_idx, name, duration);
        self.selection.select(id);
    }

    pub fn push_video_clip(&mut self, media_idx: usize, name: String, duration: f32) {
        let id = self.data.push_video_clip(media_idx, name, duration);
        self.selection.select(id);
    }

    pub fn push_audio_clip(&mut self, audio_idx: usize, name: String, duration: f32) {
        let id = self.data.push_audio_clip(audio_idx, name, duration);
        self.selection.select(id);
    }

    pub fn remove_selected_clip(&mut self) {
        if let Some(id) = self.selection.selected_clip_id {
            self.data.remove_clip(id);
            self.selection.clear();
        }
    }

    pub fn move_clip_to_start(&mut self, clip_id: usize, new_start: f32) {
        self.data.move_clip_to_start(clip_id, new_start, self.fps);
    }

    pub fn resize_clip_start_to(&mut self, clip_id: usize, new_start: f32) {
        self.data.resize_clip_start_to(clip_id, new_start, self.fps);
    }

    pub fn resize_clip_end_to(&mut self, clip_id: usize, new_end: f32) {
        self.data.resize_clip_end_to(clip_id, new_end, self.fps);
    }

    pub fn move_clip_to_track(&mut self, clip_id: usize, target_track_index: usize) {
        self.data
            .move_clip_to_track(clip_id, target_track_index, &mut self.interaction);
    }
}
