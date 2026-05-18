use super::App;
use crate::config_panel;
use crate::properties_panel;
use crate::sidebar;
use crate::transport;
use eframe::egui;

impl App {
    pub(super) fn handle_config_action(&mut self, action: config_panel::ConfigAction) {
        match action {
            config_panel::ConfigAction::SelectMidi => self.pick_midi_file(),
            config_panel::ConfigAction::AddWaterfall => {
                let duration = self.project.duration() as f32;
                let midi_idx = self.project.midi.highlighted_idx;
                self.project
                    .timeline_state
                    .push_waterfall_clip(midi_idx, duration);
            }
            config_panel::ConfigAction::AddSolidColor => {
                let duration = self.project.duration() as f32;
                let color = egui::Color32::from_rgb(200, 80, 80);
                self.project
                    .timeline_state
                    .push_solid_color_clip(color, duration);
            }
            config_panel::ConfigAction::RemoveMidi(idx) => {
                self.project.remove_midi(idx);
                self.render_ctx.reset_midi_state();
            }
        }
    }

    pub(super) fn render_side_panels(&mut self, ui: &mut egui::Ui) {
        let mut config_action = None;
        let dark_mode = self.ui.theme_mode.is_dark(ui.ctx());
        self.project.timeline_state.fps = self.project.render.fps;
        let duration = self.project.duration() as f32;

        egui::Panel::left("sidebar")
            .exact_size(60.0)
            .resizable(false)
            .show_inside(ui, |ui| {
                sidebar::show(ui, &mut self.ui.active_tab);
            });

        egui::Panel::bottom("transport")
            .exact_size(200.0)
            .resizable(false)
            .show_inside(ui, |ui| {
                let mut transport_time = self.project.playback.current_time as f32;
                transport::show(
                    ui,
                    &mut self.project.playback.is_playing,
                    &mut transport_time,
                    duration,
                    &mut self.project.timeline_state,
                    dark_mode,
                );
                self.project.playback.current_time = transport_time as f64;
            });

        egui::Panel::left("config_panel")
            .default_size(260.0)
            .min_size(180.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                let mut state = config_panel::ConfigState {
                    active_tab: self.ui.active_tab,
                    midi_files: &self.project.midi.entries,
                    highlighted_midi_idx: &mut self.project.midi.highlighted_idx,
                    render_width: &mut self.project.render.width,
                    render_height: &mut self.project.render.height,
                    fps: &mut self.project.render.fps,
                    export_format: &mut self.ui.export_format,
                    encoder: &mut self.ui.encoder,
                    export_path: &mut self.ui.export_path,
                    theme_mode: &mut self.ui.theme_mode,
                };

                if let Some(action) = config_panel::show(ui, &mut state) {
                    config_action = Some(action);
                }
            });

        egui::Panel::right("properties_panel")
            .default_size(220.0)
            .min_size(160.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                properties_panel::show(
                    ui,
                    &mut self.project.timeline_state,
                    self.ui.zoom,
                    &self.project.midi.entries,
                );
            });

        if let Some(action) = config_action {
            self.handle_config_action(action);
        }
    }
}
