use crate::transport::TimelineView;
use eframe::egui;

#[derive(Clone, Debug)]
pub struct TimelineMetrics {
    pub ruler_height: f32,
    pub scrollbar_height: f32,
    pub controls_height: f32,
    pub section_label_height: f32,
    pub section_gap: f32,
    pub clip_edge_width: f32,
    pub clip_vertical_inset: f32,
    pub clip_label_min_width: f32,
    pub clip_text_padding: f32,
    pub scrollbar_handle_width: f32,
}

impl Default for TimelineMetrics {
    fn default() -> Self {
        Self {
            ruler_height: 26.0,
            scrollbar_height: 16.0,
            controls_height: 32.0,
            section_label_height: 20.0,
            section_gap: 4.0,
            clip_edge_width: 8.0,
            clip_vertical_inset: 3.0,
            clip_label_min_width: 40.0,
            clip_text_padding: 2.0,
            scrollbar_handle_width: 6.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TimelineLayout {
    pub timeline_rect: egui::Rect,
    pub ruler_rect: egui::Rect,
    pub controls_rect: egui::Rect,
    pub scrollbar_rect: egui::Rect,
    pub content_width: f32,
    pub visible_start: f32,
    pub visible_end: f32,
    pub content_bottom: f32,
}

impl TimelineLayout {
    pub fn new(
        timeline_rect: egui::Rect,
        view: &TimelineView,
        metrics: &TimelineMetrics,
    ) -> Self {
        let content_width = (timeline_rect.width() - view.header_width).max(1.0);
        let (visible_start, visible_end) = view.visible_range(content_width);
        let ruler_rect = egui::Rect::from_min_size(
            timeline_rect.min,
            egui::vec2(timeline_rect.width(), metrics.ruler_height),
        );
        let controls_rect = egui::Rect::from_min_max(
            egui::pos2(
                timeline_rect.min.x,
                timeline_rect.max.y - metrics.controls_height,
            ),
            timeline_rect.max,
        );
        let scrollbar_rect = egui::Rect::from_min_size(
            egui::pos2(
                timeline_rect.min.x + view.header_width,
                timeline_rect.max.y - metrics.controls_height - metrics.scrollbar_height,
            ),
            egui::vec2(content_width, metrics.scrollbar_height),
        );
        let content_bottom = controls_rect.min.y - metrics.scrollbar_height;

        Self {
            timeline_rect,
            ruler_rect,
            controls_rect,
            scrollbar_rect,
            content_width,
            visible_start,
            visible_end,
            content_bottom,
        }
    }

    pub fn section_label_rect(&self, y: f32, metrics: &TimelineMetrics) -> egui::Rect {
        egui::Rect::from_min_size(
            egui::pos2(self.timeline_rect.min.x, y),
            egui::vec2(self.timeline_rect.width(), metrics.section_label_height),
        )
    }

    pub fn track_rect(&self, y: f32, track_height: f32) -> egui::Rect {
        egui::Rect::from_min_size(
            egui::pos2(self.timeline_rect.min.x, y),
            egui::vec2(self.timeline_rect.width(), track_height),
        )
    }

    pub fn header_rect(&self, track_rect: &egui::Rect, header_width: f32) -> egui::Rect {
        egui::Rect::from_min_size(track_rect.min, egui::vec2(header_width, track_rect.height()))
    }

    pub fn clip_rect(
        &self,
        view: &TimelineView,
        track_rect: &egui::Rect,
        clip_start: f32,
        clip_end: f32,
        metrics: &TimelineMetrics,
    ) -> egui::Rect {
        let x1 = view.screen_x_for_time(&self.timeline_rect, clip_start);
        let x2 = view.screen_x_for_time(&self.timeline_rect, clip_end);
        egui::Rect::from_min_max(
            egui::pos2(
                x1.max(track_rect.min.x + view.header_width),
                track_rect.min.y + metrics.clip_vertical_inset,
            ),
            egui::pos2(
                x2.min(track_rect.max.x),
                track_rect.max.y - metrics.clip_vertical_inset,
            ),
        )
    }

    pub fn content_interact_rect(&self, view: &TimelineView) -> egui::Rect {
        egui::Rect::from_min_max(
            egui::pos2(self.timeline_rect.min.x + view.header_width, self.ruler_rect.max.y),
            egui::pos2(self.timeline_rect.max.x, self.content_bottom),
        )
    }
}
