use crate::transport::TimelineDrawContext;
use crate::transport::controller::TimelineCommand;
use crate::transport::timecode::{font, format_timecode_full};
use eframe::egui;

pub fn draw_controls(ctx: &mut TimelineDrawContext<'_>, is_playing: bool, current_time: f32) {
    let controls_rect = ctx.layout.controls_rect;
    ctx.painter
        .rect_filled(controls_rect, 0.0, ctx.c.controls_bg);
    ctx.painter.rect_stroke(
        controls_rect,
        0.0,
        egui::Stroke::new(1.0, ctx.c.border),
        egui::StrokeKind::Inside,
    );

    let mut child_ui = ctx
        .ui
        .new_child(egui::UiBuilder::new().max_rect(controls_rect));
    child_ui.horizontal(|ui| {
        ui.add_space(8.0);
        if ui.button(if is_playing { "⏸" } else { "▶" }).clicked() {
            ctx.commands.push(TimelineCommand::SetPlaying(!is_playing));
        }
        if ui.button("⏹").clicked() {
            ctx.commands.push(TimelineCommand::StopPlayback);
        }
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new(format!(
                "{} / {}",
                format_timecode_full(current_time, ctx.state.fps),
                format_timecode_full(ctx.duration, ctx.state.fps),
            ))
            .font(font(12.0)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("缩放: {:.0}px/s", ctx.state.view.zoom))
                    .font(font(11.0))
                    .color(ctx.c.dim_text),
            );
        });
    });
}
