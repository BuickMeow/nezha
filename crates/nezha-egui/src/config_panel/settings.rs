//! 设置标签页 — 主题、音频设备、关于。

use crate::app::ThemeMode;
use eframe::egui;

pub fn show(
    ui: &mut egui::Ui,
    theme_mode: &mut ThemeMode,
    audio_device_name: &mut Option<String>,
    audio_devices: &[String],
) {
    ui.label("主题");
    ui.horizontal(|ui| {
        if ui
            .selectable_label(*theme_mode == ThemeMode::Light, "\u{2600} 浅色")
            .clicked()
        {
            *theme_mode = ThemeMode::Light;
        }
        if ui
            .selectable_label(*theme_mode == ThemeMode::Dark, "🌙 深色")
            .clicked()
        {
            *theme_mode = ThemeMode::Dark;
        }
        if ui
            .selectable_label(*theme_mode == ThemeMode::System, "💻 跟随系统")
            .clicked()
        {
            *theme_mode = ThemeMode::System;
        }
    });

    ui.separator();
    ui.label("音频输出设备");
    ui.add_space(4.0);

    if audio_devices.is_empty() {
        ui.colored_label(
            egui::Color32::from_rgb(255, 180, 100),
            "未检测到音频输出设备",
        );
    } else {
        let current = audio_device_name.as_deref().unwrap_or("");
        egui::ComboBox::from_id_salt("audio_device")
            .selected_text(if current.is_empty() {
                "默认设备"
            } else {
                current
            })
            .show_ui(ui, |ui| {
                for dev in audio_devices {
                    let is_selected = audio_device_name.as_deref().map_or(false, |d| d == dev);
                    if ui.selectable_label(is_selected, dev).clicked() {
                        *audio_device_name = Some(dev.clone());
                    }
                }
            });
    }

    ui.separator();
    ui.label("关于");
    ui.label("Nezha MIDI Renderer v0.1.0");
}
