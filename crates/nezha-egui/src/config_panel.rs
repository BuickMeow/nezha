use eframe::egui;
use crate::sidebar::SidebarTab;
use crate::app::ThemeMode;

pub struct ConfigState<'a> {
    pub active_tab: SidebarTab,
    pub midi_path: &'a Option<String>,
    pub render_width: &'a mut u32,
    pub render_height: &'a mut u32,
    pub fps: &'a mut u32,
    pub export_format: &'a mut String,
    pub encoder: &'a mut String,
    pub export_path: &'a mut Option<String>,
    pub theme_mode: &'a mut ThemeMode,
}

#[derive(Clone, Debug)]
pub enum ConfigAction {
    SelectMidi,
    Resize { width: u32, height: u32 },
    AddWaterfall,
    AddSolidColor,
}

pub fn show(ui: &mut egui::Ui, state: &mut ConfigState) -> Option<ConfigAction> {
    let mut action: Option<ConfigAction> = None;

    ui.heading("配置");
    ui.separator();

    match state.active_tab {
        SidebarTab::Midi => {
            ui.label("MIDI 文件");
            if ui.button("选择 MIDI 文件").clicked() {
                action = Some(ConfigAction::SelectMidi);
            }
            if let Some(path) = state.midi_path {
                ui.label(format!("已加载: {}", path));
            } else {
                ui.label("暂无文件");
            }

            ui.separator();
            ui.label("渲染设置");

            ui.horizontal(|ui| {
                ui.label("分辨率:");
                ui.add(
                    egui::DragValue::new(state.render_width).speed(1.0).range(1..=7680),
                );
                ui.label("x");
                ui.add(
                    egui::DragValue::new(state.render_height).speed(1.0).range(1..=4320),
                );
                if ui.button("应用").clicked() {
                    action = Some(ConfigAction::Resize {
                        width: *state.render_width,
                        height: *state.render_height,
                    });
                }
            });

            ui.horizontal(|ui| {
                ui.label("帧率:");
                ui.add(egui::DragValue::new(state.fps).speed(1.0).range(1..=240));
                ui.label("fps");
            });
        }
        SidebarTab::Style => {
            ui.label("添加图层到时间轴");
            ui.add_space(4.0);

            if ui.button("🌊 默认瀑布流").clicked() {
                action = Some(ConfigAction::AddWaterfall);
            }
            ui.add_space(4.0);
            if ui.button("🎨 纯色图层").clicked() {
                action = Some(ConfigAction::AddSolidColor);
            }
        }
        SidebarTab::Export => {
            ui.label("导出设置");

            ui.horizontal(|ui| {
                ui.label("渲染格式:");
                egui::ComboBox::from_id_salt("export_format")
                    .selected_text(state.export_format.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(state.export_format, "MP4".to_string(), "MP4");
                        ui.selectable_value(state.export_format, "MOV".to_string(), "MOV");
                        ui.selectable_value(state.export_format, "MKV".to_string(), "MKV");
                        ui.selectable_value(state.export_format, "AVI".to_string(), "AVI");
                    });
            });

            ui.horizontal(|ui| {
                ui.label("编码器:");
                egui::ComboBox::from_id_salt("encoder")
                    .selected_text(state.encoder.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(state.encoder, "H.264".to_string(), "H.264");
                        ui.selectable_value(state.encoder, "H.265 / HEVC".to_string(), "H.265 / HEVC");
                        ui.selectable_value(state.encoder, "ProRes".to_string(), "ProRes");
                        ui.selectable_value(state.encoder, "VP9".to_string(), "VP9");
                        ui.selectable_value(state.encoder, "AV1".to_string(), "AV1");
                    });
            });

            ui.horizontal(|ui| {
                ui.label("导出位置:");
                if let Some(path) = state.export_path {
                    ui.label(path.as_str());
                } else {
                    ui.label("未选择");
                }
                if ui.button("浏览...").clicked() {
                    if let Some(path) = rfd::FileDialog::new().save_file() {
                        *state.export_path = Some(path.to_string_lossy().to_string());
                    }
                }
            });

            ui.add_space(12.0);
            if ui.button("开始导出").clicked() {
                // TODO: 开始导出
            }
        }
        SidebarTab::Settings => {
            ui.label("主题");
            ui.horizontal(|ui| {
                if ui.selectable_label(*state.theme_mode == ThemeMode::Light, "☀️ 浅色").clicked() {
                    *state.theme_mode = ThemeMode::Light;
                }
                if ui.selectable_label(*state.theme_mode == ThemeMode::Dark, "🌙 深色").clicked() {
                    *state.theme_mode = ThemeMode::Dark;
                }
                if ui.selectable_label(*state.theme_mode == ThemeMode::System, "💻 跟随系统").clicked() {
                    *state.theme_mode = ThemeMode::System;
                }
            });

            ui.separator();
            ui.label("关于");
            ui.label("Nezha MIDI Renderer v0.1.0");
        }
    }

    action
}
