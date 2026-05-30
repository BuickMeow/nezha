use crate::transport::controller::TimelineCommand;
use crate::transport::hit_test::scrollbar_hit_areas;
use crate::transport::{ScrollbarDrag, TimelineDrawContext};
use eframe::egui;

pub fn draw_scrollbar(ctx: &mut TimelineDrawContext<'_>) {
    let scrollbar_rect = ctx.layout.scrollbar_rect;
    let content_width = ctx.layout.content_width;

    ctx.painter
        .rect_filled(scrollbar_rect, 2.0, ctx.c.scrollbar_bg);
    ctx.painter.rect_stroke(
        scrollbar_rect,
        2.0,
        egui::Stroke::new(1.0, ctx.c.border),
        egui::StrokeKind::Inside,
    );

    if ctx.duration <= 0.0 {
        return;
    }

    let (visible_start, visible_end) = ctx.state.view.visible_range(content_width);
    let vis_start = visible_start.clamp(0.0, ctx.duration);
    let vis_end = visible_end.clamp(vis_start, ctx.duration);
    let Some(hit_areas) =
        scrollbar_hit_areas(ctx.layout, ctx.metrics, ctx.duration, &ctx.state.view)
    else {
        return;
    };
    let thumb_rect = hit_areas.thumb_rect;

    ctx.painter
        .rect_filled(thumb_rect, 2.0, ctx.c.scrollbar_thumb);
    if let Some(left) = hit_areas.left_handle {
        ctx.painter.rect_filled(left, 1.0, ctx.c.scrollbar_handle);
        let center = left.center();
        let arrow = vec![
            egui::pos2(center.x + 2.0, center.y - 3.0),
            egui::pos2(center.x - 2.0, center.y),
            egui::pos2(center.x + 2.0, center.y + 3.0),
        ];
        ctx.painter.add(egui::Shape::convex_polygon(
            arrow,
            ctx.c.scrollbar_handle_arrow,
            egui::Stroke::NONE,
        ));
    }
    if let Some(right) = hit_areas.right_handle {
        ctx.painter.rect_filled(right, 1.0, ctx.c.scrollbar_handle);
        let center = right.center();
        let arrow = vec![
            egui::pos2(center.x - 2.0, center.y - 3.0),
            egui::pos2(center.x + 2.0, center.y),
            egui::pos2(center.x - 2.0, center.y + 3.0),
        ];
        ctx.painter.add(egui::Shape::convex_polygon(
            arrow,
            ctx.c.scrollbar_handle_arrow,
            egui::Stroke::NONE,
        ));
    }

    // ── 核心改动：为 thumb 创建独立的 ui.interact，避免和全局 response 竞争 ──
    let thumb_response = ctx.ui.interact(
        thumb_rect,
        egui::Id::new("scrollbar_thumb"),
        egui::Sense::drag(),
    );

    // Hover 光标
    if thumb_response.hovered()
        && let Some(pos) = ctx.ui.input(|i| i.pointer.hover_pos())
    {
        let edge_threshold = 10.0_f32.min(thumb_rect.width() / 3.0);
        let dist_left = pos.x - thumb_rect.min.x;
        let dist_right = thumb_rect.max.x - pos.x;
        if dist_left < edge_threshold {
            ctx.ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeWest);
        } else if dist_right < edge_threshold {
            ctx.ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeEast);
        } else {
            ctx.ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    // 拖拽中光标
    if let Some(drag) = &ctx.state.interaction.scrollbar_drag {
        match drag {
            ScrollbarDrag::Pan { .. } => {
                ctx.ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
            ScrollbarDrag::LeftEdge => {
                ctx.ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeWest);
            }
            ScrollbarDrag::RightEdge => {
                ctx.ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeEast);
            }
        }
    }

    // 拖拽开始
    if thumb_response.drag_started()
        && let Some(pos) = thumb_response.interact_pointer_pos()
    {
        let edge_threshold = 10.0_f32.min(thumb_rect.width() / 3.0);
        let dist_left = pos.x - thumb_rect.min.x;
        let dist_right = thumb_rect.max.x - pos.x;

        if dist_left < edge_threshold {
            ctx.commands.push(TimelineCommand::SetScrollbarDrag(Some(
                ScrollbarDrag::LeftEdge,
            )));
        } else if dist_right < edge_threshold {
            ctx.commands.push(TimelineCommand::SetScrollbarDrag(Some(
                ScrollbarDrag::RightEdge,
            )));
        } else {
            let rel_x = (pos.x - scrollbar_rect.min.x).clamp(0.0, scrollbar_rect.width());
            ctx.commands.push(TimelineCommand::SetScrollbarDrag(Some(
                ScrollbarDrag::Pan {
                    anchor_time: rel_x / scrollbar_rect.width() * ctx.duration,
                    anchor_vis_start: vis_start,
                },
            )));
        }
    }

    // 拖拽结束
    if !thumb_response.dragged_by(egui::PointerButton::Primary) {
        ctx.commands.push(TimelineCommand::SetScrollbarDrag(None));
    }

    // 拖拽过程
    if thumb_response.dragged()
        && let Some(drag) = &ctx.state.interaction.scrollbar_drag
        && let Some(pos) = thumb_response.interact_pointer_pos()
    {
        let rel_x = (pos.x - scrollbar_rect.min.x).clamp(0.0, scrollbar_rect.width());
        let mouse_time = rel_x / scrollbar_rect.width() * ctx.duration;

        match drag {
            ScrollbarDrag::Pan {
                anchor_time,
                anchor_vis_start,
            } => {
                let time_offset = mouse_time - anchor_time;
                let visible_dur = vis_end - vis_start;
                ctx.commands.push(TimelineCommand::SetScrollOffset(
                    (anchor_vis_start + time_offset)
                        .clamp(0.0, (ctx.duration - visible_dur).max(0.0)),
                ));
            }
            ScrollbarDrag::LeftEdge => {
                let new_start = mouse_time.clamp(0.0, vis_end - 1.0 / ctx.fps.max(1) as f32);
                let new_zoom = content_width / (vis_end - new_start);
                ctx.commands.push(TimelineCommand::SetZoomAndScroll {
                    zoom: new_zoom,
                    scroll_offset: new_start,
                });
            }
            ScrollbarDrag::RightEdge => {
                let new_end =
                    mouse_time.clamp(vis_start + 1.0 / ctx.fps.max(1) as f32, ctx.duration);
                let new_zoom = content_width / (new_end - vis_start);
                ctx.commands.push(TimelineCommand::SetZoomAndScroll {
                    zoom: new_zoom,
                    scroll_offset: vis_start,
                });
            }
        }
    }
}
