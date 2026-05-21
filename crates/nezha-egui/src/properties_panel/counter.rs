//! 计数器图层的属性面板 — 特有属性。

use crate::transport::TrackClip;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, clip: &mut TrackClip) {
    ui.heading("计数器");
    ui.add_space(2.0);

    // ── 字体 ──
    ui.label("字号");
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(&mut clip.font_size, 8..=128)
                .step_by(1.0)
                .text("px"),
        );
    });
    ui.label(
        egui::RichText::new(format!("当前: {} px", clip.font_size))
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );

    ui.add_space(8.0);

    // ── 文字颜色 ──
    ui.label("文字颜色");
    let mut rgb = [clip.color.r(), clip.color.g(), clip.color.b()];
    ui.color_edit_button_srgb(&mut rgb);
    clip.color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);

    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("计数器会自动显示当前播放时间和可见音符总数。位置与合成方式在上方「变换 / 合成」中配置。")
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );
}
