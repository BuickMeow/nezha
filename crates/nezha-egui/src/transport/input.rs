use eframe::egui;
use crate::transport::TimelineView;

pub fn handle_input(
    ui: &egui::Ui,
    response: &egui::Response,
    view: &mut TimelineView,
    timeline_rect: &egui::Rect,
) {
    if !response.hovered() {
        return;
    }

    let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
    let scroll_x = ui.input(|i| i.smooth_scroll_delta.x);
    let zoom_delta = ui.input(|i| i.zoom_delta());

    if scroll_y != 0.0 {
        if let Some(mouse_pos) = response.hover_pos() {
            view.zoom_around_pointer(timeline_rect, mouse_pos.x, 1.0 + scroll_y * 0.001);
        }
    }

    if scroll_x != 0.0 {
        view.pan_by_pixels(scroll_x);
    }

    if zoom_delta != 1.0 {
        if let Some(mouse_pos) = response.hover_pos() {
            view.zoom_around_pointer(timeline_rect, mouse_pos.x, zoom_delta);
        }
    }
}
