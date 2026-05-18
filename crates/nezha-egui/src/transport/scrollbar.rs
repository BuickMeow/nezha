use eframe::egui;
use crate::transport::controller::TimelineCommand;
use crate::transport::hit_test::{scrollbar_hit_areas, ScrollbarHitTarget};
use crate::transport::layout::{TimelineLayout, TimelineMetrics};
use crate::transport::{TimelineView, TimelineInteraction, ScrollbarDrag, ThemeColors};

pub fn draw_scrollbar(
    _ui: &egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    layout: &TimelineLayout,
    metrics: &TimelineMetrics,
    view: &TimelineView,
    interaction: &TimelineInteraction,
    duration: f32,
    response: &egui::Response,
    fps: u32,
    commands: &mut Vec<TimelineCommand>,
) {
    let scrollbar_rect = layout.scrollbar_rect;
    let content_width = layout.content_width;

    painter.rect_filled(scrollbar_rect, 2.0, c.scrollbar_bg);
    painter.rect_stroke(scrollbar_rect, 2.0, egui::Stroke::new(1.0, c.border), egui::StrokeKind::Inside);

    if duration <= 0.0 {
        return;
    }

    let (visible_start, visible_end) = view.visible_range(content_width);
    let vis_start = visible_start.clamp(0.0, duration);
    let vis_end = visible_end.clamp(vis_start, duration);
    let Some(hit_areas) = scrollbar_hit_areas(layout, metrics, duration, view) else {
        return;
    };
    let thumb_rect = hit_areas.thumb_rect;

    painter.rect_filled(thumb_rect, 2.0, c.scrollbar_thumb);
    painter.rect_filled(hit_areas.left_handle, 1.0, c.scrollbar_handle);
    painter.rect_filled(hit_areas.right_handle, 1.0, c.scrollbar_handle);

    // 交互
    if response.drag_started_by(egui::PointerButton::Primary)
        && interaction.scrollbar_drag.is_none()
        && !interaction.dragging_playhead
    {
        if let Some(pos) = response.interact_pointer_pos() {
            if scrollbar_rect.contains(pos) {
                if let Some(hit) = hit_areas.target_at(pos, &scrollbar_rect, duration) {
                    commands.push(TimelineCommand::SetScrollbarDrag(Some(match hit {
                        ScrollbarHitTarget::Pan { anchor_time } => ScrollbarDrag::Pan {
                            anchor_time,
                        },
                        ScrollbarHitTarget::LeftEdge => ScrollbarDrag::LeftEdge,
                        ScrollbarHitTarget::RightEdge => ScrollbarDrag::RightEdge,
                    })));
                }
            }
        }
    }

    if !response.dragged_by(egui::PointerButton::Primary) {
        commands.push(TimelineCommand::SetScrollbarDrag(None));
    }

    if let Some(drag) = &interaction.scrollbar_drag {
        if let Some(pos) = response.interact_pointer_pos() {
            let rel_x = (pos.x - scrollbar_rect.min.x).clamp(0.0, scrollbar_rect.width());
            let mouse_time = rel_x / scrollbar_rect.width() * duration;

            match drag {
                ScrollbarDrag::Pan { anchor_time } => {
                    let time_offset = mouse_time - anchor_time;
                    let visible_dur = vis_end - vis_start;
                    commands.push(TimelineCommand::SetScrollOffset(
                        (vis_start + time_offset).clamp(0.0, (duration - visible_dur).max(0.0)),
                    ));
                }
                ScrollbarDrag::LeftEdge => {
                    let new_start = mouse_time.clamp(0.0, vis_end - 1.0 / fps.max(1) as f32);
                    let new_zoom = content_width / (vis_end - new_start);
                    commands.push(TimelineCommand::SetZoomAndScroll {
                        zoom: new_zoom,
                        scroll_offset: new_start,
                    });
                }
                ScrollbarDrag::RightEdge => {
                    let new_end = mouse_time.clamp(vis_start + 1.0 / fps.max(1) as f32, duration);
                    let new_zoom = content_width / (new_end - vis_start);
                    commands.push(TimelineCommand::SetZoom(new_zoom));
                }
            }
        }
    }
}
