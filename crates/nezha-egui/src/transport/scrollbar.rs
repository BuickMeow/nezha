use eframe::egui;
use crate::transport::{TimelineView, TimelineInteraction, ScrollbarDrag, ThemeColors};

pub fn draw_scrollbar(
    _ui: &egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    timeline_rect: &egui::Rect,
    view: &mut TimelineView,
    interaction: &mut TimelineInteraction,
    duration: f32,
    content_width: f32,
    scrollbar_height: f32,
    controls_height: f32,
    response: &egui::Response,
    fps: u32,
) {
    let scrollbar_y = timeline_rect.max.y - controls_height - scrollbar_height;
    let scrollbar_rect = egui::Rect::from_min_size(
        egui::pos2(timeline_rect.min.x + view.header_width, scrollbar_y),
        egui::vec2(content_width, scrollbar_height),
    );

    painter.rect_filled(scrollbar_rect, 2.0, c.scrollbar_bg);
    painter.rect_stroke(scrollbar_rect, 2.0, egui::Stroke::new(1.0, c.border), egui::StrokeKind::Inside);

    if duration <= 0.0 {
        return;
    }

    let visible_start = view.scroll_offset;
    let visible_end = visible_start + content_width / view.zoom;
    let vis_start = visible_start.clamp(0.0, duration);
    let vis_end = visible_end.clamp(vis_start, duration);

    let thumb_x1 = scrollbar_rect.min.x + (vis_start / duration) * scrollbar_rect.width();
    let thumb_x2 = scrollbar_rect.min.x + (vis_end / duration) * scrollbar_rect.width();
    let thumb_rect = egui::Rect::from_min_max(
        egui::pos2(thumb_x1.max(scrollbar_rect.min.x), scrollbar_rect.min.y + 2.0),
        egui::pos2(thumb_x2.min(scrollbar_rect.max.x), scrollbar_rect.max.y - 2.0),
    );

    painter.rect_filled(thumb_rect, 2.0, c.scrollbar_thumb);

    let handle_w = 6.0;
    let left_handle = egui::Rect::from_min_max(
        egui::pos2(thumb_rect.min.x, thumb_rect.min.y),
        egui::pos2(thumb_rect.min.x + handle_w, thumb_rect.max.y),
    );
    let right_handle = egui::Rect::from_min_max(
        egui::pos2(thumb_rect.max.x - handle_w, thumb_rect.min.y),
        egui::pos2(thumb_rect.max.x, thumb_rect.max.y),
    );

    painter.rect_filled(left_handle, 1.0, c.scrollbar_handle);
    painter.rect_filled(right_handle, 1.0, c.scrollbar_handle);

    // 交互
    if response.drag_started_by(egui::PointerButton::Primary)
        && interaction.scrollbar_drag.is_none()
        && !interaction.dragging_playhead
    {
        if let Some(pos) = response.interact_pointer_pos() {
            if scrollbar_rect.contains(pos) {
                if left_handle.contains(pos) {
                    interaction.scrollbar_drag = Some(ScrollbarDrag::LeftEdge);
                } else if right_handle.contains(pos) {
                    interaction.scrollbar_drag = Some(ScrollbarDrag::RightEdge);
                } else if thumb_rect.contains(pos) {
                    let rel_x = (pos.x - scrollbar_rect.min.x).clamp(0.0, scrollbar_rect.width());
                    let anchor_time = rel_x / scrollbar_rect.width() * duration;
                    interaction.scrollbar_drag = Some(ScrollbarDrag::Pan { anchor_time });
                }
            }
        }
    }

    if !response.dragged_by(egui::PointerButton::Primary) {
        interaction.scrollbar_drag = None;
    }

    if let Some(drag) = &interaction.scrollbar_drag {
        if let Some(pos) = response.interact_pointer_pos() {
            let rel_x = (pos.x - scrollbar_rect.min.x).clamp(0.0, scrollbar_rect.width());
            let mouse_time = rel_x / scrollbar_rect.width() * duration;

            match drag {
                ScrollbarDrag::Pan { anchor_time } => {
                    let time_offset = mouse_time - anchor_time;
                    let visible_dur = vis_end - vis_start;
                    view.scroll_offset = (vis_start + time_offset)
                        .clamp(0.0, (duration - visible_dur).max(0.0));
                }
                ScrollbarDrag::LeftEdge => {
                    let new_start = mouse_time.clamp(0.0, vis_end - 1.0 / fps.max(1) as f32);
                    let new_zoom = content_width / (vis_end - new_start);
                    view.zoom = new_zoom.clamp(0.2, 5000.0);
                    view.scroll_offset = new_start;
                }
                ScrollbarDrag::RightEdge => {
                    let new_end = mouse_time.clamp(vis_start + 1.0 / fps.max(1) as f32, duration);
                    let new_zoom = content_width / (new_end - vis_start);
                    view.zoom = new_zoom.clamp(0.2, 5000.0);
                }
            }
        }
    }
}
