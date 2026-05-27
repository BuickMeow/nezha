use super::App;
use crate::config_panel;
use crate::properties_panel;
use crate::sidebar;
use crate::transport;
use eframe::egui;
use std::path::PathBuf;

impl App {
    pub(super) fn handle_config_action(&mut self, action: config_panel::ConfigAction) {
        match action {
            config_panel::ConfigAction::SelectMidi => self.pick_midi_file(),
            config_panel::ConfigAction::AddWaterfall => {
                self.add_waterfall_with_audio_prompt();
            }
            config_panel::ConfigAction::AddSolidColor => {
                let duration = self.project.duration() as f32;
                let color = egui::Color32::from_rgb(200, 80, 80);
                self.project
                    .timeline_state
                    .push_solid_color_clip(color, duration);
            }
            config_panel::ConfigAction::AddCounter => {
                let duration = self.project.duration() as f32;
                let midi_idx = self.project.midi.highlighted_idx;
                self.project.timeline_state.push_counter_clip(duration, midi_idx);
            }
            config_panel::ConfigAction::RemoveMidi(idx) => {
                self.project.remove_midi(idx);
                self.render_ctx.reset_midi_state();
            }
            config_panel::ConfigAction::StartExport => {
                self.start_export();
            }
            // SoundFont actions
            config_panel::ConfigAction::AddSoundfont => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SoundFont", &["sf2", "sfz"])
                    .pick_file()
                {
                    self.project
                        .soundfonts
                        .push(crate::app::project_state::SoundFontEntry { path });
                }
            }
            config_panel::ConfigAction::RemoveSoundfont(idx) => {
                if idx < self.project.soundfonts.len() {
                    self.project.soundfonts.remove(idx);
                }
            }
            config_panel::ConfigAction::MoveSoundfontUp(idx) => {
                if idx > 0 && idx < self.project.soundfonts.len() {
                    self.project.soundfonts.swap(idx, idx - 1);
                }
            }
            config_panel::ConfigAction::MoveSoundfontDown(idx) => {
                if idx + 1 < self.project.soundfonts.len() {
                    self.project.soundfonts.swap(idx, idx + 1);
                }
            }
            config_panel::ConfigAction::RenderAudio(midi_idx) => {
                if let Some(entry) = self.project.midi.entries.get(midi_idx) {
                    let name = PathBuf::from(&entry.path)
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("MIDI")
                        .to_string();
                    self.audio_manager.prepare_render(name, entry.path.clone());
                }
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
                sidebar::show(
                    ui,
                    &mut self.ui.active_tab,
                    &mut self.ui.config_panel_visible,
                );
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

        if self.ui.config_panel_visible {
            egui::Panel::left("config_panel")
                .exact_size(260.0)
                .min_size(260.0)
                .max_size(260.0)
                .resizable(false)
                .show_inside(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    ui.set_min_width(ui.available_width());

                    let mut state = config_panel::ConfigState {
                        active_tab: self.ui.active_tab,
                        midi_files: &self.project.midi.entries,
                        highlighted_midi_idx: &mut self.project.midi.highlighted_idx,
                        render_width: &mut self.project.render.width,
                        render_height: &mut self.project.render.height,
                        fps: &mut self.project.render.fps,
                        export_format: &mut self.ui.export_format,
                        encoder: &mut self.ui.encoder,
                        encoder_backend: &mut self.ui.encoder_backend,
                        export_path: &mut self.ui.export_path,
                        theme_mode: &mut self.ui.theme_mode,
                        soundfonts: &self.project.soundfonts,
                        audio_device_name: &mut self.ui.audio_device_name,
                        audio_devices: &self.ui.audio_devices,
                    };

                    if let Some(action) = config_panel::show(ui, &mut state) {
                        config_action = Some(action);
                    }
                });
        }

        // Properties panel: only show when a clip is selected
        if self
            .project
            .timeline_state
            .selection
            .selected_clip_id
            .is_some()
        {
            egui::Panel::right("properties_panel")
                .exact_size(220.0)
                .min_size(220.0)
                .max_size(220.0)
                .resizable(false)
                .show_inside(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    ui.set_min_width(ui.available_width());

                    properties_panel::show(
                        ui,
                        &mut self.project.timeline_state,
                        self.ui.zoom,
                        &self.project.midi.entries,
                    );
                });
        }

        if let Some(action) = config_action {
            self.handle_config_action(action);
        }
    }
}
