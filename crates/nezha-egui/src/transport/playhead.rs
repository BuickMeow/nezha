use crate::transport::controller::TimelineCommand;
use crate::transport::hit_test::{is_content_hit, playhead_hit_rect};
use crate::transport::timecode::snap_to_frame;
use crate::transport::TimelineDrawContext;
use eframe::egui;

pub fn draw_playhead(ctx: &mut TimelineDrawContext<'_>, current_time: f32) {
    let timeline_rect = ctx.layout.timeline_rect;
    let playhead_x = ctx.state.view.screen_x_for_time(&timeline_rect, current_time);
    let hit_rect = playhead_hit_rect(ctx.layout, &ctx.state.view, current_time);
    let hovering_playhead = ctx.response.hover_pos().is_some_and(|p| hit_rect.contains(p));

    if ctx.response.drag_started_by(egui::PointerButton::Primary)
        && hovering_playhead
        && !ctx.ui.input(|i| i.modifiers.shift)
        && ctx.state.interaction.scrollbar_drag.is_none()
    {
        ctx.commands.push(TimelineCommand::SetPlayheadDragging(true));
    }
    if !ctx.response.dragged_by(egui::PointerButton::Primary) {
        ctx.commands.push(TimelineCommand::SetPlayheadDragging(false));
    }

    if ctx.state.interaction.dragging_playhead
        && let Some(mouse_pos) = ctx.response.interact_pointer_pos()
    {
        let new_time = ctx.state.view.time_at_screen_x(&timeline_rect, mouse_pos.x);
        ctx.commands.push(TimelineCommand::SetCurrentTime(
            snap_to_frame(new_time, ctx.fps).clamp(0.0, ctx.duration),
        ));
    }

    // 空白内容区点击跳转（response.clicked_by 在 Clip 上不触发，
    // 避免干扰选择图层）
    if ctx.response.clicked_by(egui::PointerButton::Primary)
        && !hovering_playhead
        && !ctx.ui.input(|i| i.modifiers.shift)
        && !ctx.state.interaction.dragging_playhead
        && ctx.state.interaction.scrollbar_drag.is_none()
        && let Some(mouse_pos) = ctx.response.hover_pos()
        && is_content_hit(ctx.layout, &ctx.state.view, mouse_pos)
    {
        let new_time = ctx.state.view.time_at_screen_x(&timeline_rect, mouse_pos.x);
        ctx.commands.push(TimelineCommand::SetCurrentTime(
            snap_to_frame(new_time, ctx.fps).clamp(0.0, ctx.duration),
        ));
    }

    if playhead_x >= timeline_rect.min.x + ctx.state.view.header_width {
        ctx.painter.line_segment(
            [
                egui::pos2(playhead_x, timeline_rect.min.y),
                egui::pos2(playhead_x, ctx.layout.controls_rect.min.y),
            ],
            egui::Stroke::new(2.0, ctx.c.playhead),
        );
        let tri = vec![
            egui::pos2(playhead_x - 7.0, timeline_rect.min.y),
            egui::pos2(playhead_x + 7.0, timeline_rect.min.y),
            egui::pos2(playhead_x, timeline_rect.min.y + 9.0),
        ];
        ctx.painter.add(egui::Shape::convex_polygon(
            tri,
            ctx.c.playhead,
            egui::Stroke::NONE,
        ));
    }
}
