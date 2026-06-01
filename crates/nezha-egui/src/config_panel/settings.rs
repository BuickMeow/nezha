//! 设置标签页 — 主题、语言、音频设备、关于。

use crate::app::ThemeMode;
use crate::config_panel::ConfigAction;
use eframe::egui;
use rust_i18n::t;

pub fn show(
    ui: &mut egui::Ui,
    theme_mode: &mut ThemeMode,
    locale: &mut String,
    audio_device_name: &mut Option<String>,
    audio_devices: &[String],
) -> Option<ConfigAction> {
    ui.label(t!("settings.theme"));
    ui.horizontal(|ui| {
        if ui
            .selectable_label(*theme_mode == ThemeMode::Light, format!("\u{2600} {}", t!("settings.theme.light")))
            .clicked()
        {
            *theme_mode = ThemeMode::Light;
        }
        if ui
            .selectable_label(*theme_mode == ThemeMode::Dark, format!("🌙 {}", t!("settings.theme.dark")))
            .clicked()
        {
            *theme_mode = ThemeMode::Dark;
        }
        if ui
            .selectable_label(*theme_mode == ThemeMode::System, format!("💻 {}", t!("settings.theme.system")))
            .clicked()
        {
            *theme_mode = ThemeMode::System;
        }
    });

    ui.separator();
    ui.label(t!("settings.language"));
    let mut locale_changed = false;
    ui.horizontal(|ui| {
        if ui
            .selectable_label(*locale == "auto", t!("settings.language.auto"))
            .clicked()
        {
            *locale = "auto".to_string();
            rust_i18n::set_locale(crate::config::resolve_locale("auto"));
            locale_changed = true;
        }
        if ui
            .selectable_label(*locale == "zh-CN", "中文")
            .clicked()
        {
            *locale = "zh-CN".to_string();
            rust_i18n::set_locale("zh-CN");
            locale_changed = true;
        }
        if ui
            .selectable_label(*locale == "en-US", "English")
            .clicked()
        {
            *locale = "en-US".to_string();
            rust_i18n::set_locale("en-US");
            locale_changed = true;
        }
    });

    ui.separator();
    ui.label(t!("settings.audio_device"));
    ui.add_space(4.0);

    if audio_devices.is_empty() {
        ui.colored_label(
            egui::Color32::from_rgb(255, 180, 100),
            t!("settings.audio_device.none"),
        );
    } else {
        let current = audio_device_name.as_deref().unwrap_or("");
        egui::ComboBox::from_id_salt("audio_device")
            .selected_text(if current.is_empty() {
                t!("settings.audio_device.default")
            } else {
                current.into()
            })
            .show_ui(ui, |ui| {
                for dev in audio_devices {
                    let is_selected = audio_device_name.as_deref().is_some_and(|d| d == dev);
                    if ui.selectable_label(is_selected, dev).clicked() {
                        *audio_device_name = Some(dev.clone());
                    }
                }
            });
    }

    ui.separator();
    ui.label(t!("settings.about"));
    ui.label("Nezha MIDI Renderer v0.1.0");

    if locale_changed {
        Some(ConfigAction::LocaleChanged)
    } else {
        None
    }
}
