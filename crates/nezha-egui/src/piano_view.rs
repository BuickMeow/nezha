use eframe::egui;

pub fn show(
    ui: &mut egui::Ui,
    texture_id: egui::TextureId,
    available: egui::Vec2,
    aspect: f32,
    zoom: &mut f32,
    pan_offset: &mut egui::Vec2,
) {
    let container_aspect = available.x / available.y.max(0.001);

    let base_size = if container_aspect > aspect {
        egui::Vec2::new(available.y * aspect, available.y)
    } else {
        egui::Vec2::new(available.x, available.x / aspect)
    };

    let (_rect, response) =
        ui.allocate_exact_size(available, egui::Sense::click_and_drag());

    let pointer_in_rect = response.hovered();

    if pointer_in_rect {
        let zoom_delta = ui.input(|i| i.zoom_delta());
        if zoom_delta != 1.0 {
            *zoom *= zoom_delta;
        }

        let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
        if scroll_delta.y > 0.0 {
            *zoom *= 1.1;
        } else if scroll_delta.y < 0.0 {
            *zoom /= 1.1;
        }
    }

    *zoom = zoom.clamp(1.0, 10.0);

    let scaled_size = base_size * *zoom;

    let excess = scaled_size - available;
    let max_pan = egui::Vec2::new(
        (excess.x / 2.0).max(0.0),
        (excess.y / 2.0).max(0.0),
    );

    if response.dragged() {
        *pan_offset += response.drag_delta();
    }

    pan_offset.x = pan_offset.x.clamp(-max_pan.x, max_pan.x);
    pan_offset.y = pan_offset.y.clamp(-max_pan.y, max_pan.y);

    if response.double_clicked() {
        *zoom = 1.0;
        *pan_offset = egui::Vec2::ZERO;
    }

    let center = available / 2.0;
    let top_left = center - scaled_size / 2.0 + *pan_offset;

    let image_rect = egui::Rect::from_min_size(
        ui.min_rect().min + top_left,
        scaled_size,
    );

    ui.put(
        image_rect,
        egui::Image::new(egui::load::SizedTexture::new(texture_id, scaled_size)),
    );

    if *zoom > 1.01 {
        let zoom_text = format!("{:.0}%", *zoom * 100.0);
        let text_pos = ui.min_rect().min
            + egui::Vec2::new(available.x - 50.0, available.y - 20.0);
        ui.painter().text(
            text_pos,
            egui::Align2::RIGHT_BOTTOM,
            zoom_text,
            egui::FontId::proportional(12.0),
            ui.visuals().text_color(),
        );
    }
}
