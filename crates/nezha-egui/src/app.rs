use eframe::egui;

mod archive_picker;
mod audio_coordinator;
mod audio_manager;
mod audio_player;
pub(crate) mod constants;
pub(crate) mod error;
mod export;
mod export_controller;
mod file_loader;
mod loading;
mod media_ops;
mod panels;
mod playback;
mod preview;
mod preview_layer;
pub mod project_state;
mod preview_renderer;
mod render_context;
mod ui_state;

use audio_coordinator::AudioCoordinator;
use export_controller::ExportController;
use file_loader::{FileLoader, MidiLoadResult};
pub use project_state::ProjectState;
use preview_renderer::PreviewRenderer;
pub use ui_state::{ThemeMode, UiState};

pub struct App {
    pub(crate) renderer: PreviewRenderer,
    pub project: ProjectState,
    pub ui: UiState,
    pub(crate) audio: AudioCoordinator,
    pub(crate) export: ExportController,
    pub(crate) files: FileLoader,
    /// 上次保存的配置 JSON，用于避免每帧重复写磁盘。
    last_saved_config: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(feature = "profiling")]
        {
            puffin::set_scopes_on(true);
            let _ = std::mem::ManuallyDrop::new(
                puffin_http::Server::new(&format!(
                    "0.0.0.0:{}",
                    nezha_renderer::constants::PUFFIN_PORT
                ))
                .expect("puffin_http"),
            );
            tracing::info!("Puffin bridge on :8585");
        }

        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "MiSans".to_owned(),
            egui::FontData::from_static(include_bytes!("../../../assets/MiSans-Regular.otf"))
                .into(),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "MiSans".to_owned());
        cc.egui_ctx.set_fonts(fonts);

        let theme_mode = ThemeMode::System;
        theme_mode.apply(&cc.egui_ctx);

        // Load config early to set locale before ProjectState::new() creates tracks.
        let cfg = crate::config::Config::load();
        rust_i18n::set_locale(crate::config::resolve_locale(&cfg.locale));

        let mut app = Self {
            renderer: PreviewRenderer::new(cc),
            project: ProjectState::new(),
            ui: UiState::default(),
            audio: AudioCoordinator::new(),
            export: ExportController::new(),
            files: FileLoader::new(),
            last_saved_config: String::new(),
        };

        app.last_saved_config = serde_json::to_string(&cfg).unwrap_or_default();
        cfg.apply(&mut app.ui, &mut app.project);
        app.ui.refresh_audio_devices();
        app
    }

    pub fn on_midi_loaded(&mut self, path: String, midi: nezha_core::MidiFile) {
        self.project.insert_midi(path.clone(), midi);
        let file_name = std::path::Path::new(&path)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("MIDI")
            .to_string();
        self.audio.manager.prepare_render(file_name, path);
    }

    fn save_config(&mut self) {
        let cfg = crate::config::Config::from_ui(&self.ui, &self.project);
        let json = serde_json::to_string(&cfg).unwrap_or_default();
        if json != self.last_saved_config {
            self.last_saved_config = json;
            cfg.save();
        }
    }

    fn add_waterfall_with_audio_prompt(&mut self) {
        let midi_idx = self.project.midi.highlighted_idx;
        let pre_song = crate::app::constants::DEFAULT_PRE_SONG_BUFFER;
        let (duration, song_start, song_dur) = midi_idx
            .and_then(|idx| self.project.midi.entries.get(idx))
            .map(|e| {
                let d = e.file.duration as f32;
                (pre_song + d, pre_song, d)
            })
            .unwrap_or_else(|| (self.project.duration() as f32, 0.0, 0.0));
        self.project
            .timeline_state
            .push_waterfall_clip(midi_idx, duration, song_start, song_dur);
        if let Some(idx) = midi_idx
            && let Some(entry) = self.project.midi.entries.get(idx)
        {
            let name = std::path::Path::new(&entry.path)
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("MIDI")
                .to_string();
            self.audio.manager.prepare_render(name, entry.path.clone());
        }
    }

    fn show_error_toast(&mut self, ui: &mut egui::Ui) {
        if let Some(ref err) = self.project.last_error {
            let err_msg = err.to_string();
            let mut dismissed = false;
            let screen_rect = ui.ctx().content_rect();
            egui::Area::new("error_toast".into())
                .fixed_pos(egui::pos2(screen_rect.center().x, 32.0))
                .anchor(egui::Align2::CENTER_TOP, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .fill(egui::Color32::from_rgb(60, 30, 30))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&err_msg)
                                        .color(egui::Color32::from_rgb(255, 180, 100)),
                                );
                                if ui.button("✕").clicked() {
                                    dismissed = true;
                                }
                            });
                        });
                });
            if dismissed {
                self.project.last_error = None;
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(feature = "profiling")]
        puffin::GlobalProfiler::lock().new_frame();

        self.ui.theme_mode.apply(ui.ctx());
        self.handle_input(ui);

        // Audio: poll render thread + sync playback
        self.audio.poll(&mut self.project);
        self.audio.sync(&mut self.project, &self.ui);

        // Export pipeline
        if self.export.has_export() {
            self.export_step();
        }

        // MIDI loading
        match self.files.poll_midi_loading(&mut self.project) {
            MidiLoadResult::Loaded { path, midi } => {
                self.on_midi_loaded(path, midi);
                self.renderer.reset_midi_state();
            }
            MidiLoadResult::NotReady => {}
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.render_side_panels(ui);
            egui::CentralPanel::default().show_inside(ui, |ui| {
                self.render_preview(ui);
            });
            ui.ctx().request_repaint();
        });

        self.save_config();

        self.audio.show_dialog(ui, &mut self.project);
        self.files.show_midi_loading_overlay(ui);
        self.show_archive_picker(ui);
        self.show_error_toast(ui);
        self.show_export_overlay(ui);
    }
}
