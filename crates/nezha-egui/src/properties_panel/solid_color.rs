//! 纯色图层的属性面板。

use crate::transport::TrackClip;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, clip: &mut TrackClip) {
    ui.add_space(4.0);
    ui.label("颜色");
    let mut rgb = [clip.color.r(), clip.color.g(), clip.color.b()];
    ui.color_edit_button_srgb(&mut rgb);
    clip.color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
}
