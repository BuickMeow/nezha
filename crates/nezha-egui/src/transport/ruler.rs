use crate::transport::TimelineDrawContext;
use crate::transport::controller::TimelineCommand;
use crate::transport::hit_test::is_ruler_hit;
use crate::transport::timecode::{
    font, format_timecode_frames, format_timecode_seconds, snap_to_frame,
};
use eframe::egui;

pub fn draw_ruler(ctx: &mut TimelineDrawContext<'_>, _current_time: f32) {
    let ruler_rect = ctx.layout.ruler_rect;
    let visible_start = ctx.layout.visible_start;
    let visible_end = ctx.layout.visible_end;
    ctx.painter.rect_filled(ruler_rect, 0.0, ctx.c.ruler_bg);
    ctx.painter.rect_stroke(
        ruler_rect,
        0.0,
        egui::Stroke::new(1.0, ctx.c.border),
        egui::StrokeKind::Inside,
    );

    // 点击 ruler 跳转（使用原始点击事件，不受子组件干扰）
    // is_ruler_hit 确保只有标尺区域生效，不影响轨道操作
    if ctx.ui.input(|i| i.pointer.primary_clicked())
        && !ctx.ui.input(|i| i.modifiers.shift)
        && !ctx.state.interaction.dragging_playhead
        && ctx.state.interaction.scrollbar_drag.is_none()
        && let Some(mouse_pos) = ctx.ui.input(|i| i.pointer.interact_pos())
        && is_ruler_hit(ctx.layout, &ctx.state.view, mouse_pos)
    {
        let timeline_rect = ctx.layout.timeline_rect;
        let new_time = ctx.state.view.time_at_screen_x(&timeline_rect, mouse_pos.x);
        ctx.commands.push(TimelineCommand::SetCurrentTime(
            snap_to_frame(new_time, ctx.fps).clamp(0.0, ctx.duration),
        ));
    }

    let frame_interval = 1.0 / ctx.fps.max(1) as f32;

    let thresholds = [
        (5000.0, frame_interval),
        (3000.0, 2.0 * frame_interval),
        (1500.0, 5.0 * frame_interval),
        (750.0, 10.0 * frame_interval),
        (300.0, 30.0 * frame_interval),
        (150.0, 60.0 * frame_interval),
        (100.0, 2.0),
        (50.0, 5.0),
        (20.0, 10.0),
        (10.0, 30.0),
        (5.0, 60.0),
        (2.0, 120.0),
    ];
    let major_interval = thresholds
        .iter()
        .find(|(z, _)| ctx.state.view.zoom > *z)
        .map(|(_, interval)| *interval)
        .unwrap_or(300.0);

    let timeline_rect = ctx.layout.timeline_rect;

    // 主刻度
    let mut t = (visible_start / major_interval).floor() * major_interval;
    while t <= visible_end {
        let x = ctx.state.view.screen_x_for_time(&timeline_rect, t);
        if x >= timeline_rect.min.x + ctx.state.view.header_width {
            ctx.painter.line_segment(
                [
                    egui::pos2(x, ruler_rect.min.y + 14.0),
                    egui::pos2(x, ruler_rect.max.y),
                ],
                egui::Stroke::new(1.0, ctx.c.ruler_tick),
            );
            let label = if major_interval < 1.0 {
                format_timecode_frames(t, ctx.fps)
            } else {
                format_timecode_seconds(t)
            };
            ctx.painter.text(
                egui::pos2(x + 3.0, ruler_rect.min.y + 2.0),
                egui::Align2::LEFT_TOP,
                label,
                font(11.0),
                ctx.c.ruler_text,
            );
        }
        t += major_interval;
    }

    // 次刻度（帧级别）
    if ctx.state.view.zoom > 3000.0 {
        let mut ft = (visible_start / frame_interval).floor() * frame_interval;
        while ft <= visible_end {
            let x = ctx.state.view.screen_x_for_time(&timeline_rect, ft);
            if x >= timeline_rect.min.x + ctx.state.view.header_width {
                let is_major = ((ft / major_interval).round() * major_interval - ft).abs() < 0.001;
                if !is_major {
                    ctx.painter.line_segment(
                        [
                            egui::pos2(x, ruler_rect.min.y + 20.0),
                            egui::pos2(x, ruler_rect.max.y),
                        ],
                        egui::Stroke::new(1.0, ctx.c.ruler_tick.gamma_multiply(0.5)),
                    );
                }
            }
            ft += frame_interval;
        }
    }
}
