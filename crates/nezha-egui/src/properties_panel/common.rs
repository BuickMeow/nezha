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
                .range(-7680.0..=7680.0)
                .suffix(" px"),
        );
        ui.label("Y:");
        ui.add(
            egui::DragValue::new(&mut common.position_y)
                .speed(1.0)
                .range(-4320.0..=4320.0)
                .suffix(" px"),
        );
    });

    ui.add_space(4.0);

    // ── 缩放（带链接按钮）──
    ui.label("缩放");
    ui.horizontal(|ui| {
        // 链接按钮
        let link_text = if common.scale_linked { "🔗" } else { "🔓" };
        if ui
            .selectable_label(common.scale_linked, link_text)
            .on_hover_text(if common.scale_linked {
                "已锁定横纵比"
            } else {
                "未锁定横纵比"
            })
            .clicked()
        {
            common.scale_linked = !common.scale_linked;
            if common.scale_linked {
                // 锁定时以当前 scale_x 为准同步 scale_y
                common.scale_y = common.scale_x;
            }
        }

        ui.label("W:");
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

        ui.label("H:");
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
            egui::RichText::new("💡 负缩放 = 翻转（部分渲染器可能尚未支持）")
                .size(11.0)
                .color(egui::Color32::from_rgb(255, 200, 100)),
        );
    }

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
