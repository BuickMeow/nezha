//! 项目标签页 — 渲染设置。

use eframe::egui;

pub fn show(ui: &mut egui::Ui, render_width: &mut u32, render_height: &mut u32, fps: &mut u32) {
    ui.label("渲染设置");

    ui.horizontal(|ui| {
        ui.label("分辨率:");
        ui.add(
            egui::DragValue::new(render_width)
                .speed(1.0)
                .range(1..=7680),
        );
        ui.label("x");
        ui.add(
            egui::DragValue::new(render_height)
                .speed(1.0)
                .range(1..=4320),
        );
    });

    ui.horizontal(|ui| {
        ui.label("帧率:");
        ui.add(egui::DragValue::new(fps).speed(1.0).range(1..=240));
        ui.label("fps");
    });
}
