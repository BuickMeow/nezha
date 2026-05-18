use eframe::egui;
pub mod project_state;
mod loading;
mod panels;
mod playback;
mod preview;
mod render_context;
mod ui_state;

use loading::MidiLoader;
pub use project_state::ProjectState;
pub use render_context::RenderContext;
pub use ui_state::{ThemeMode, UiState};

pub struct App {
    pub render_ctx: RenderContext,
    pub project: ProjectState,
    pub ui: UiState,
    midi_loader: Option<MidiLoader>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(feature = "profiling")]
        {
            puffin::set_scopes_on(true);
            // Leak the server so it lives for the entire app lifetime
            let _ = std::mem::ManuallyDrop::new(
                puffin_http::Server::new("0.0.0.0:8585").expect("puffin_http"),
            );
            println!("🔥 Puffin bridge on :8585 → puffin_viewer --url 127.0.0.1:8585");
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

        Self {
            render_ctx: RenderContext::new(cc, 1920, 1080),
            project: ProjectState::new(),
            ui: UiState::default(),
            midi_loader: None,
        }
    }

    fn show_error_toast(&mut self, ui: &mut egui::Ui) {
        if let Some(err) = self.project.last_error.clone() {
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
                                    egui::RichText::new(&err)
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

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.render_side_panels(ui);

            egui::CentralPanel::default().show_inside(ui, |ui| {
                self.render_preview(ui);
            });

            ui.ctx().request_repaint();
        });

        self.show_midi_loading(ui);
        self.show_error_toast(ui);
    }
}
