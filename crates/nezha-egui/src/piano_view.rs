use eframe::egui;

/// 最小缩放倍数
const MIN_ZOOM: f32 = 1.0;
/// 最大缩放倍数
const MAX_ZOOM: f32 = 10.0;
/// 滚轮缩放系数
const ZOOM_SCROLL_FACTOR: f32 = 1.1;

/// 显示瀑布流预览纹理（琴键已由渲染器绘制在纹理内部）
pub fn show(
    ui: &mut egui::Ui,
    texture_id: egui::TextureId,
    available: egui::Vec2,
    aspect: f32,
    zoom: &mut f32,
    pan_offset: &mut egui::Vec2,
) -> f32 {
    let container_aspect = available.x / available.y.max(0.001);

    let base_size = if container_aspect > aspect {
        egui::Vec2::new(available.y * aspect, available.y)
    } else {
        egui::Vec2::new(available.x, available.x / aspect)
    };

    let (_rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());

    if response.hovered() {
        let old_zoom = *zoom;

        let zoom_delta = ui.input(|i| i.zoom_delta());
        if zoom_delta != 1.0 {
            *zoom *= zoom_delta;
        }

        let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
        if scroll_delta.y > 0.0 {
            *zoom *= ZOOM_SCROLL_FACTOR;
        } else if scroll_delta.y < 0.0 {
            *zoom /= ZOOM_SCROLL_FACTOR;
        }

        *zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);

        // 以鼠标位置为中心缩放
        if *zoom != old_zoom {
            if let Some(cursor) = pointer_pos {
                let center = available / 2.0;
                let cursor_rel = cursor - response.rect.min - center;
                *pan_offset += cursor_rel * (1.0 - old_zoom / *zoom);
            }
        }
    }

    let scaled_size = base_size * *zoom;

    let excess = scaled_size - available;
    let max_pan = egui::Vec2::new((excess.x / 2.0).max(0.0), (excess.y / 2.0).max(0.0));

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

    let image_rect = egui::Rect::from_min_size(response.rect.min + top_left, scaled_size);

    ui.put(
        image_rect,
        egui::Image::new(egui::load::SizedTexture::new(texture_id, scaled_size)),
    );

    *zoom
}
