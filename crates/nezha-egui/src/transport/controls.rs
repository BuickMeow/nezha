use eframe::egui;
use crate::transport::{TimelineState, ThemeColors};
use crate::transport::timecode::{format_timecode_full, font};

pub fn draw_controls(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    timeline_rect: &egui::Rect,
    is_playing: &mut bool,
    current_time: &mut f32,
    duration: f32,
    state: &TimelineState,
    controls_height: f32,
) {
    let controls_rect = egui::Rect::from_min_max(
        egui::pos2(timeline_rect.min.x, timeline_rect.max.y - controls_height),
        timeline_rect.max,
    );
    painter.rect_filled(controls_rect, 0.0, c.controls_bg);
    painter.rect_stroke(controls_rect, 0.0, egui::Stroke::new(1.0, c.border), egui::StrokeKind::Inside);

    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(controls_rect));
    child_ui.horizontal(|ui| {
        ui.add_space(8.0);
        if ui.button(if *is_playing { "⏸" } else { "▶" }).clicked() {
            *is_playing = !*is_playing;
        }
        if ui.button("⏹").clicked() {
            *is_playing = false;
            *current_time = 0.0;
        }
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new(format!(
                "{} / {}",
                format_timecode_full(*current_time, state.fps),
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
