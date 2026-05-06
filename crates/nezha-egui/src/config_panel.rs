use eframe::egui;
use crate::sidebar::SidebarTab;

pub fn show(
    ui: &mut egui::Ui,
    active_tab: SidebarTab,
    midi_path: &Option<String>,
    on_select_midi: &mut dyn FnMut(),
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
                ui.label("分辨率:");
                ui.label("1920×1080");
            });
            ui.horizontal(|ui| {
                ui.label("帧率:");
                ui.label("60 fps");
            });
            if ui.button("开始导出").clicked() {
                // TODO: 开始导出
            }
        }
    }
}
