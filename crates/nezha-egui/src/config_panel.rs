use eframe::egui;
use crate::sidebar::SidebarTab;

pub fn show(
    ui: &mut egui::Ui,
    active_tab: SidebarTab,
    midi_path: &Option<String>,
    on_select_midi: &mut dyn FnMut(),
    render_width: &mut u32,
    render_height: &mut u32,
    fps: &mut u32,
    needs_resize: &mut bool,
    export_format: &mut String,
    encoder: &mut String,
    export_path: &mut Option<String>,
) {
    ui.heading("配置");
    ui.separator();

    match active_tab {
        SidebarTab::Midi => {
            ui.label("MIDI 文件");
            if ui.button("选择 MIDI 文件").clicked() {
                on_select_midi();
            }
            if let Some(path) = midi_path {
                ui.label(format!("已加载: {}", path));
            } else {
                ui.label("暂无文件");
            }

            ui.separator();
            ui.label("渲染设置");

            ui.horizontal(|ui| {
                ui.label("分辨率:");
                ui.add(
                    egui::DragValue::new(render_width).speed(1.0).range(1..=7680),
                );
                ui.label("x");
                ui.add(
                    egui::DragValue::new(render_height).speed(1.0).range(1..=4320),
                );
                if ui.button("应用").clicked() {
                    *needs_resize = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("帧率:");
                ui.add(egui::DragValue::new(fps).speed(1.0).range(1..=240));
                ui.label("fps");
            });
        }
        SidebarTab::Style => {
            ui.label("样式设置");
            ui.group(|ui| {
                ui.label("背景颜色");
                ui.color_edit_button_srgb(&mut [0, 0, 0]);
            });
            ui.group(|ui| {
                ui.label("音符颜色");
                ui.color_edit_button_srgb(&mut [100, 150, 255]);
            });
        }
        SidebarTab::Export => {
            ui.label("导出设置");

            ui.horizontal(|ui| {
                ui.label("渲染格式:");
                egui::ComboBox::from_id_salt("export_format")
                    .selected_text(export_format.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(export_format, "MP4".to_string(), "MP4");
                        ui.selectable_value(export_format, "MOV".to_string(), "MOV");
                        ui.selectable_value(export_format, "MKV".to_string(), "MKV");
                        ui.selectable_value(export_format, "AVI".to_string(), "AVI");
                    });
            });

            ui.horizontal(|ui| {
                ui.label("编码器:");
                egui::ComboBox::from_id_salt("encoder")
                    .selected_text(encoder.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(encoder, "H.264".to_string(), "H.264");
                        ui.selectable_value(encoder, "H.265 / HEVC".to_string(), "H.265 / HEVC");
                        ui.selectable_value(encoder, "ProRes".to_string(), "ProRes");
                        ui.selectable_value(encoder, "VP9".to_string(), "VP9");
                        ui.selectable_value(encoder, "AV1".to_string(), "AV1");
                    });
            });

            ui.horizontal(|ui| {
                ui.label("导出位置:");
                if let Some(path) = export_path {
                    ui.label(path.as_str());
                } else {
                    ui.label("未选择");
                }
                if ui.button("浏览...").clicked() {
                    if let Some(path) = rfd::FileDialog::new().save_file() {
                        *export_path = Some(path.to_string_lossy().to_string());
                    }
                }
            });

            ui.add_space(12.0);
            if ui.button("开始导出").clicked() {
                // TODO: 开始导出
            }
        }
    }
}
