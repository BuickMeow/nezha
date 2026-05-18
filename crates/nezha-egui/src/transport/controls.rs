use eframe::egui;
use crate::transport::controller::TimelineCommand;
use crate::transport::layout::TimelineLayout;
use crate::transport::{TimelineState, ThemeColors};
use crate::transport::timecode::{format_timecode_full, font};

pub fn draw_controls(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    layout: &TimelineLayout,
    is_playing: bool,
    current_time: f32,
    duration: f32,
    state: &TimelineState,
    commands: &mut Vec<TimelineCommand>,
) {
    let controls_rect = layout.controls_rect;
    painter.rect_filled(controls_rect, 0.0, c.controls_bg);
    painter.rect_stroke(controls_rect, 0.0, egui::Stroke::new(1.0, c.border), egui::StrokeKind::Inside);

    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(controls_rect));
    child_ui.horizontal(|ui| {
        ui.add_space(8.0);
        if ui.button(if is_playing { "⏸" } else { "▶" }).clicked() {
            commands.push(TimelineCommand::SetPlaying(!is_playing));
        }
        if ui.button("⏹").clicked() {
            commands.push(TimelineCommand::StopPlayback);
        }
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new(format!(
                "{} / {}",
                format_timecode_full(current_time, state.fps),
                format_timecode_full(duration, state.fps),
            ))
            .font(font(12.0)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("缩放: {:.0}px/s", state.view.zoom))
                    .font(font(11.0))
                    .color(c.dim_text),
            );
        });
    });
}
