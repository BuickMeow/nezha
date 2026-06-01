//! 所有图层共有的变换与合成属性 UI。

use crate::transport::LayerCommon;
use eframe::egui;
use rust_i18n::t;

pub fn show_common(ui: &mut egui::Ui, common: &mut LayerCommon) {
    ui.heading(t!("common.transform"));
    ui.add_space(2.0);

    // ── 位置 ──
    ui.label(t!("common.position"));
    ui.horizontal(|ui| {
        ui.label(t!("common.x"));
        ui.add(
            egui::DragValue::new(&mut common.position_x)
                .speed(1.0)
                .range(-7680.0..=7680.0)
                .suffix(" px"),
        );
        ui.label(t!("common.y"));
        ui.add(
            egui::DragValue::new(&mut common.position_y)
                .speed(1.0)
                .range(-4320.0..=4320.0)
                .suffix(" px"),
        );
    });

    ui.add_space(4.0);

    // ── 缩放（带链接按钮）──
    ui.label(t!("common.scale"));
    ui.horizontal(|ui| {
        // 链接按钮
        let link_text = if common.scale_linked { "🔗" } else { "🔓" };
        if ui
            .selectable_label(common.scale_linked, link_text)
            .on_hover_text(if common.scale_linked {
                t!("common.scale.locked")
            } else {
                t!("common.scale.unlocked")
            })
            .clicked()
        {
            common.scale_linked = !common.scale_linked;
            if common.scale_linked {
                // 锁定时以当前 scale_x 为准同步 scale_y
                common.scale_y = common.scale_x;
            }
        }

        ui.label(t!("common.w"));
        let prev_x = common.scale_x;
        let resp_x = ui.add(
            egui::DragValue::new(&mut common.scale_x)
                .speed(0.01)
                .range(-10.0..=10.0)
                .fixed_decimals(2),
        );
        if resp_x.changed() && common.scale_linked {
            let delta = common.scale_x / prev_x;
            common.scale_y = (common.scale_y * delta * 100.0).round() / 100.0;
        }

        ui.label(t!("common.h"));
        let prev_y = common.scale_y;
        let resp_y = ui.add(
            egui::DragValue::new(&mut common.scale_y)
                .speed(0.01)
                .range(-10.0..=10.0)
                .fixed_decimals(2),
        );
        if resp_y.changed() && common.scale_linked && !resp_x.changed() {
            let delta = common.scale_y / prev_y;
            common.scale_x = (common.scale_x * delta * 100.0).round() / 100.0;
        }
    });
    if common.scale_x < 0.0 || common.scale_y < 0.0 {
        ui.label(
            egui::RichText::new(format!("💡 {}", t!("common.scale.negative")))
                .size(11.0)
                .color(egui::Color32::from_rgb(255, 200, 100)),
        );
    }

    ui.add_space(6.0);
    ui.separator();
    ui.heading(t!("common.composition"));
    ui.add_space(2.0);

    // ── 不透明度 ──
    ui.label(t!("common.opacity"));
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
    ui.label(t!("common.blend_mode"));
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

fn blend_mode_label(mode: crate::transport::BlendMode) -> String {
    match mode {
        crate::transport::BlendMode::Normal => t!("common.blend_mode.normal"),
        crate::transport::BlendMode::Add => t!("common.blend_mode.add"),
        crate::transport::BlendMode::Multiply => t!("common.blend_mode.multiply"),
    }
    .to_string()
}
