use eframe::egui;

#[derive(Clone, Debug)]
pub struct TimelineView {
    pub zoom: f32,
    pub scroll_offset: f32,
    pub scroll_y: f32,
    pub track_height: f32,
    pub header_width: f32,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self {
            zoom: 50.0,
            scroll_offset: 0.0,
            scroll_y: 0.0,
            track_height: 36.0,
            header_width: 100.0,
        }
    }
}

impl TimelineView {
    pub fn visible_range(&self, content_width: f32) -> (f32, f32) {
        let visible_start = self.scroll_offset;
        let visible_end = visible_start + content_width / self.zoom;
        (visible_start, visible_end)
    }

    pub fn time_at_screen_x(&self, timeline_rect: &egui::Rect, x: f32) -> f32 {
        (x - timeline_rect.min.x - self.header_width) / self.zoom + self.scroll_offset
    }

    pub fn screen_x_for_time(&self, timeline_rect: &egui::Rect, time: f32) -> f32 {
        timeline_rect.min.x + self.header_width + (time - self.scroll_offset) * self.zoom
    }

    pub fn zoom_around_pointer(
        &mut self,
        timeline_rect: &egui::Rect,
        pointer_x: f32,
        zoom_factor: f32,
    ) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * zoom_factor).clamp(0.2, 5000.0);
        let mouse_time =
            (pointer_x - timeline_rect.min.x - self.header_width) / old_zoom + self.scroll_offset;
        self.scroll_offset =
            mouse_time - (pointer_x - timeline_rect.min.x - self.header_width) / self.zoom;
        self.clamp_scroll();
    }

    pub fn pan_by_pixels(&mut self, pixels: f32) {
        self.scroll_offset -= pixels / self.zoom;
        self.clamp_scroll();
    }

    pub fn clamp_scroll(&mut self) {
        self.scroll_offset = self.scroll_offset.max(0.0);
    }

    pub fn clamp_scroll_y(&mut self, track_area_height: f32, total_track_height: f32) {
        let bottom_margin = self.track_height * 0.5;
        let max_scroll = (total_track_height + bottom_margin - track_area_height).max(0.0);
        self.scroll_y = self.scroll_y.clamp(0.0, max_scroll);
    }
}
