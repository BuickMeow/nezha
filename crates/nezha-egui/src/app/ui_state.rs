use eframe::egui;
use crate::sidebar::SidebarTab;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

impl ThemeMode {
    pub fn is_dark(&self, ctx: &egui::Context) -> bool {
        match self {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => ctx.global_style().visuals.dark_mode,
        }
    }

    pub fn apply(&self, ctx: &egui::Context) {
        match self {
            ThemeMode::Dark => ctx.set_theme(egui::ThemePreference::Dark),
            ThemeMode::Light => ctx.set_theme(egui::ThemePreference::Light),
            ThemeMode::System => ctx.set_theme(egui::ThemePreference::System),
        }
    }
}

pub struct UiState {
    pub active_tab: SidebarTab,
    pub export_format: String,
    pub encoder: String,
    pub export_path: Option<String>,
    pub theme_mode: ThemeMode,
    pub zoom: f32,
    pub pan_offset: egui::Vec2,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            active_tab: SidebarTab::Midi,
            export_format: "MP4".to_string(),
            encoder: "H.264".to_string(),
            export_path: None,
            theme_mode: ThemeMode::System,
            zoom: 1.0,
            pan_offset: egui::Vec2::ZERO,
        }
    }
}
