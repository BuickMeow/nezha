use crate::transport::layout::{TimelineLayout, TimelineMetrics};
use crate::transport::TimelineView;
use eframe::egui;

#[derive(Clone, Copy, Debug)]
pub struct ClipHitAreas {
    pub clip_rect: egui::Rect,
    pub left_edge: egui::Rect,
    pub right_edge: egui::Rect,
    pub middle_rect: egui::Rect,
}

pub fn clip_hit_areas(
    layout: &TimelineLayout,
    metrics: &TimelineMetrics,
    view: &TimelineView,
    track_rect: &egui::Rect,
    clip_start: f32,
    clip_end: f32,
) -> ClipHitAreas {
    let clip_rect = layout.clip_rect(view, track_rect, clip_start, clip_end, metrics);
    let left_edge = egui::Rect::from_min_size(
        clip_rect.min,
        egui::vec2(metrics.clip_edge_width, clip_rect.height()),
    );
    let right_edge = egui::Rect::from_min_size(
        egui::pos2(clip_rect.max.x - metrics.clip_edge_width, clip_rect.min.y),
        egui::vec2(metrics.clip_edge_width, clip_rect.height()),
    );
    let middle_rect = egui::Rect::from_min_max(
        egui::pos2(clip_rect.min.x + metrics.clip_edge_width, clip_rect.min.y),
        egui::pos2(clip_rect.max.x - metrics.clip_edge_width, clip_rect.max.y),
    );

    ClipHitAreas {
        clip_rect,
        left_edge,
        right_edge,
        middle_rect,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ScrollbarHitAreas {
    pub thumb_rect: egui::Rect,
    pub left_handle: egui::Rect,
    pub right_handle: egui::Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollbarHitTarget {
    Pan { anchor_time: f32 },
    LeftEdge,
    RightEdge,
}

impl ScrollbarHitAreas {
    pub fn target_at(
        &self,
        pointer_pos: egui::Pos2,
        scrollbar_rect: &egui::Rect,
        duration: f32,
    ) -> Option<ScrollbarHitTarget> {
        if self.left_handle.contains(pointer_pos) {
            Some(ScrollbarHitTarget::LeftEdge)
        } else if self.right_handle.contains(pointer_pos) {
            Some(ScrollbarHitTarget::RightEdge)
        } else if self.thumb_rect.contains(pointer_pos) {
            let rel_x = (pointer_pos.x - scrollbar_rect.min.x).clamp(0.0, scrollbar_rect.width());
            Some(ScrollbarHitTarget::Pan {
                anchor_time: rel_x / scrollbar_rect.width() * duration,
            })
        } else {
            None
        }
    }
}

pub fn scrollbar_hit_areas(
    layout: &TimelineLayout,
    metrics: &TimelineMetrics,
    duration: f32,
    view: &TimelineView,
) -> Option<ScrollbarHitAreas> {
    if duration <= 0.0 {
        return None;
    }

    let scrollbar_rect = layout.scrollbar_rect;
    let content_width = layout.content_width;
    let (visible_start, visible_end) = view.visible_range(content_width);
    let vis_start = visible_start.clamp(0.0, duration);
    let vis_end = visible_end.clamp(vis_start, duration);
    let thumb_x1 = scrollbar_rect.min.x + (vis_start / duration) * scrollbar_rect.width();
    let thumb_x2 = scrollbar_rect.min.x + (vis_end / duration) * scrollbar_rect.width();
    let thumb_rect = egui::Rect::from_min_max(
        egui::pos2(thumb_x1.max(scrollbar_rect.min.x), scrollbar_rect.min.y + 2.0),
        egui::pos2(thumb_x2.min(scrollbar_rect.max.x), scrollbar_rect.max.y - 2.0),
    );
    let left_handle = egui::Rect::from_min_max(
        egui::pos2(thumb_rect.min.x, thumb_rect.min.y),
        egui::pos2(
            thumb_rect.min.x + metrics.scrollbar_handle_width,
            thumb_rect.max.y,
        ),
    );
    let right_handle = egui::Rect::from_min_max(
        egui::pos2(
            thumb_rect.max.x - metrics.scrollbar_handle_width,
            thumb_rect.min.y,
        ),
        egui::pos2(thumb_rect.max.x, thumb_rect.max.y),
    );

    Some(ScrollbarHitAreas {
        thumb_rect,
        left_handle,
        right_handle,
    })
}

pub fn playhead_hit_rect(
    layout: &TimelineLayout,
    view: &TimelineView,
    current_time: f32,
) -> egui::Rect {
    let playhead_x = view.screen_x_for_time(&layout.timeline_rect, current_time);
    egui::Rect::from_center_size(
        egui::pos2(playhead_x, layout.timeline_rect.center().y),
        egui::vec2(10.0, layout.timeline_rect.height()),
    )
}

pub fn is_ruler_hit(layout: &TimelineLayout, view: &TimelineView, pointer_pos: egui::Pos2) -> bool {
    layout.ruler_rect.contains(pointer_pos)
        && pointer_pos.x > layout.timeline_rect.min.x + view.header_width
}

pub fn is_content_hit(layout: &TimelineLayout, view: &TimelineView, pointer_pos: egui::Pos2) -> bool {
    layout.content_interact_rect(view).contains(pointer_pos)
        && pointer_pos.x > layout.timeline_rect.min.x + view.header_width
}
