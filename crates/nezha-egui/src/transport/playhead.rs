use eframe::egui;
use crate::transport::controller::TimelineCommand;
use crate::transport::layout::{TimelineLayout, TimelineMetrics};
use crate::transport::hit_test::{is_content_hit, playhead_hit_rect};
use crate::transport::{TimelineState, ThemeColors};
use crate::transport::timecode::snap_to_frame;

pub fn draw_playhead(
    ui: &egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    layout: &TimelineLayout,
    _metrics: &TimelineMetrics,
    response: &egui::Response,
    state: &TimelineState,
    current_time: f32,
    duration: f32,
    fps: u32,
    commands: &mut Vec<TimelineCommand>,
) {
    let timeline_rect = layout.timeline_rect;
    let playhead_x = state.view.screen_x_for_time(&timeline_rect, current_time);
    let hit_rect = playhead_hit_rect(layout, &state.view, current_time);
    let hovering_playhead = response.hover_pos().is_some_and(|p| hit_rect.contains(p));

    if response.drag_started_by(egui::PointerButton::Primary)
        && hovering_playhead
        && !ui.input(|i| i.modifiers.shift)
        && state.interaction.scrollbar_drag.is_none()
    {
        commands.push(TimelineCommand::SetPlayheadDragging(true));
    }
    if !response.dragged_by(egui::PointerButton::Primary) {
        commands.push(TimelineCommand::SetPlayheadDragging(false));
    }

    if state.interaction.dragging_playhead {
        if let Some(mouse_pos) = response.interact_pointer_pos() {
            let new_time = state.view.time_at_screen_x(&timeline_rect, mouse_pos.x);
            commands.push(TimelineCommand::SetCurrentTime(
                snap_to_frame(new_time, fps).clamp(0.0, duration),
            ));
        }
    }

    if response.clicked_by(egui::PointerButton::Primary)
        && !hovering_playhead
        && !ui.input(|i| i.modifiers.shift)
        && !state.interaction.dragging_playhead
        && state.interaction.scrollbar_drag.is_none()
    {
        if let Some(mouse_pos) = response.hover_pos() {
            if is_content_hit(layout, &state.view, mouse_pos) {
                let new_time = state.view.time_at_screen_x(&timeline_rect, mouse_pos.x);
                commands.push(TimelineCommand::SetCurrentTime(
                    snap_to_frame(new_time, fps).clamp(0.0, duration),
                ));
            }
        }
    }

    if playhead_x >= timeline_rect.min.x + state.view.header_width {
        painter.line_segment(
            [
                egui::pos2(playhead_x, timeline_rect.min.y),
                egui::pos2(playhead_x, layout.controls_rect.min.y),
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
