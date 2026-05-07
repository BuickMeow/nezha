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
        let old_zoom = view.zoom;
        view.zoom = (view.zoom * (1.0 + scroll_y * 0.001)).clamp(0.2, 5000.0);
        if let Some(mouse_pos) = response.hover_pos() {
            let mouse_time = (mouse_pos.x - timeline_rect.min.x - view.header_width) / old_zoom
                + view.scroll_offset;
            view.scroll_offset = mouse_time
                - (mouse_pos.x - timeline_rect.min.x - view.header_width) / view.zoom;
        }
    }

    if scroll_x != 0.0 {
        view.scroll_offset -= scroll_x / view.zoom;
    }

    if zoom_delta != 1.0 {
        let old_zoom = view.zoom;
        view.zoom = (view.zoom * zoom_delta).clamp(0.2, 5000.0);
        if let Some(mouse_pos) = response.hover_pos() {
            let mouse_time = (mouse_pos.x - timeline_rect.min.x - view.header_width) / old_zoom
                + view.scroll_offset;
            view.scroll_offset = mouse_time
                - (mouse_pos.x - timeline_rect.min.x - view.header_width) / view.zoom;
        }
    }

    view.scroll_offset = view.scroll_offset.max(0.0);
}
