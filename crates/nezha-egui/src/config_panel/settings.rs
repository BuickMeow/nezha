//! 设置标签页 — 主题、关于。

use crate::app::ThemeMode;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, theme_mode: &mut ThemeMode) {
    ui.label("主题");
    ui.horizontal(|ui| {
        if ui
            .selectable_label(*theme_mode == ThemeMode::Light, "☀️ 浅色")
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
    ui.label("关于");
    ui.label("Nezha MIDI Renderer v0.1.0");
}
