use crate::sidebar::SidebarTab;
use cpal::traits::{DeviceTrait, HostTrait};
use eframe::egui;

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
    pub config_panel_visible: bool,
    pub export_format: String,
    pub encoder: String,
    pub encoder_backend: String,
    pub export_path: Option<String>,
    pub theme_mode: ThemeMode,
    pub zoom: f32,
    pub pan_offset: egui::Vec2,
    // 音频输出设备
    pub audio_device_name: Option<String>,
    pub audio_devices: Vec<String>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            active_tab: SidebarTab::Style,
            config_panel_visible: true,
            export_format: "MP4".to_string(),
            encoder: "H.264".to_string(),
            encoder_backend: "Software (CPU)".to_string(),
            export_path: None,
            theme_mode: ThemeMode::System,
            zoom: 1.0,
            pan_offset: egui::Vec2::ZERO,
            audio_device_name: None,
            audio_devices: Vec::new(),
        }
    }
}

impl UiState {
    /// 枚举系统中的音频输出设备，并返回设备名称列表。
    /// 如果当前选中的设备仍然存在，则保持选中；否则选中默认设备。
    pub fn refresh_audio_devices(&mut self) {
        let devices: Vec<String> = cpal::default_host()
            .output_devices()
            .ok()
            .into_iter()
            .flat_map(|devices| devices.filter_map(|d| d.name().ok()).collect::<Vec<_>>())
            .collect();

        let prev_selection = self.audio_device_name.clone();
        self.audio_devices = devices;

        // 保持选中之前选中的设备（如果还在的话）
        if let Some(ref prev) = prev_selection
            && self.audio_devices.iter().any(|d| d == prev)
        {
            self.audio_device_name = Some(prev.clone());
            return;
        }
        // 默认选中第一个设备
        self.audio_device_name = self.audio_devices.first().cloned();
    }
}
