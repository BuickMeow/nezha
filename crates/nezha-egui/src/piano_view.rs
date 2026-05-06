use eframe::egui;

pub fn show(
    ui: &mut egui::Ui,
    texture_id: egui::TextureId,
    available: egui::Vec2,
    aspect: f32,
) {
    let container_aspect = available.x / available.y.max(0.001);

    let size = if container_aspect > aspect {
        egui::Vec2::new(available.y * aspect, available.y)
    } else {
        egui::Vec2::new(available.x, available.x / aspect)
    };

    ui.centered_and_justified(|ui| {
        ui.image(egui::ImageSource::Texture(egui::load::SizedTexture::new(
            texture_id,
            size,
        )));
    });
}
