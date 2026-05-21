//! 所有图层共有的变换与合成属性 UI。

use crate::transport::LayerCommon;
use eframe::egui;

pub fn show_common(ui: &mut egui::Ui, common: &mut LayerCommon) {
    ui.heading("变换");
    ui.add_space(2.0);

    // ── 位置 ──
    ui.label("位置");
    ui.horizontal(|ui| {
        ui.label("X:");
        ui.add(
            egui::DragValue::new(&mut common.position_x)
                .speed(1.0)
                .range(0.0..=7680.0)
                .suffix(" px"),
        );
        ui.label("Y:");
        ui.add(
            egui::DragValue::new(&mut common.position_y)
                .speed(1.0)
                .range(0.0..=4320.0)
                .suffix(" px"),
        );
    });

    ui.add_space(4.0);

    // ── 缩放 ──
    ui.label("缩放");
    ui.horizontal(|ui| {
        ui.label("W:");
        ui.add(
            egui::DragValue::new(&mut common.scale_x)
                .speed(0.01)
                .range(0.01..=10.0)
                .fixed_decimals(2),
        );
        ui.label("H:");
        ui.add(
            egui::DragValue::new(&mut common.scale_y)
                .speed(0.01)
                .range(0.01..=10.0)
                .fixed_decimals(2),
        );
    });

    ui.add_space(6.0);
    ui.separator();
    ui.heading("合成");
    ui.add_space(2.0);

    // ── 不透明度 ──
    ui.label("不透明度");
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(&mut common.opacity, 0.0..=1.0)
                .step_by(0.01)
                .text(""),
        );
    });
    ui.label(
        egui::RichText::new(format!("{:.0}%", common.opacity * 100.0))
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );

    ui.add_space(4.0);

    // ── 合成方式 ──
    ui.label("合成方式");
    egui::ComboBox::from_id_salt("blend_mode_common")
        .selected_text(blend_mode_label(common.blend_mode))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for mode in &[
                crate::transport::BlendMode::Normal,
                crate::transport::BlendMode::Add,
                crate::transport::BlendMode::Multiply,
            ] {
                let label = blend_mode_label(*mode);
                if ui
                    .selectable_label(common.blend_mode == *mode, label)
                    .clicked()
                {
                    common.blend_mode = *mode;
                }
            }
        });
}

fn blend_mode_label(mode: crate::transport::BlendMode) -> &'static str {
    match mode {
        crate::transport::BlendMode::Normal => "正常",
        crate::transport::BlendMode::Add => "相加（Add）",
        crate::transport::BlendMode::Multiply => "正片叠底（Multiply）",
    }
}
