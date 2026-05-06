use eframe::egui;

pub fn show(ui: &mut egui::Ui, texture_id: egui::TextureId, size: egui::Vec2) {
    ui.centered_and_justified(|ui| {
        ui.image(egui::ImageSource::Texture(egui::load::SizedTexture::new(
            texture_id,
            size,
        )));
    });
}
