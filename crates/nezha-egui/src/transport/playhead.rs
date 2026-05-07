use eframe::egui;
use crate::transport::{TimelineState, ThemeColors};
use crate::transport::timecode::snap_to_frame;

pub fn draw_playhead(
    ui: &egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    timeline_rect: &egui::Rect,
    response: &egui::Response,
    state: &mut TimelineState,
    current_time: &mut f32,
    duration: f32,
    ruler_height: f32,
    controls_height: f32,
    scrollbar_height: f32,
    fps: u32,
) {
    let playhead_x = timeline_rect.min.x
        + state.view.header_width
        + (*current_time - state.view.scroll_offset) * state.view.zoom;

    let playhead_hit_rect = egui::Rect::from_center_size(
        egui::pos2(playhead_x, timeline_rect.center().y),
        egui::vec2(10.0, timeline_rect.height()),
    );
    let hovering_playhead = response.hover_pos().map_or(false, |p| playhead_hit_rect.contains(p));

    if response.drag_started_by(egui::PointerButton::Primary)
        && hovering_playhead
        && !ui.input(|i| i.modifiers.shift)
        && state.interaction.scrollbar_drag.is_none()
    {
        state.interaction.dragging_playhead = true;
    }
    if !response.dragged_by(egui::PointerButton::Primary) {
        state.interaction.dragging_playhead = false;
    }

    if state.interaction.dragging_playhead {
        if let Some(mouse_pos) = response.interact_pointer_pos() {
            let new_time = (mouse_pos.x - timeline_rect.min.x - state.view.header_width)
                / state.view.zoom
                + state.view.scroll_offset;
            *current_time = snap_to_frame(new_time, fps).clamp(0.0, duration);
        }
    }

    if response.clicked_by(egui::PointerButton::Primary)
        && !hovering_playhead
        && !ui.input(|i| i.modifiers.shift)
        && !state.interaction.dragging_playhead
        && state.interaction.scrollbar_drag.is_none()
    {
        if let Some(mouse_pos) = response.hover_pos() {
            if mouse_pos.x > timeline_rect.min.x + state.view.header_width
                && mouse_pos.y > timeline_rect.min.y + ruler_height
                && mouse_pos.y < timeline_rect.max.y - controls_height - scrollbar_height
            {
                let new_time = (mouse_pos.x - timeline_rect.min.x - state.view.header_width)
                    / state.view.zoom
                    + state.view.scroll_offset;
                *current_time = snap_to_frame(new_time, fps).clamp(0.0, duration);
            }
        }
    }

    if playhead_x >= timeline_rect.min.x + state.view.header_width {
        painter.line_segment(
            [
                egui::pos2(playhead_x, timeline_rect.min.y),
                egui::pos2(playhead_x, timeline_rect.max.y - controls_height),
            ],
            egui::Stroke::new(2.0, c.playhead),
        );
        let tri = vec![
            egui::pos2(playhead_x - 7.0, timeline_rect.min.y),
            egui::pos2(playhead_x + 7.0, timeline_rect.min.y),
            egui::pos2(playhead_x, timeline_rect.min.y + 9.0),
        ];
        painter.add(egui::Shape::convex_polygon(tri, c.playhead, egui::Stroke::NONE));
    }
}
