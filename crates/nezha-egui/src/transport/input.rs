use crate::transport::TimelineView;
use crate::transport::layout::TimelineLayout;
use eframe::egui;

pub fn handle_input(
    ui: &egui::Ui,
    response: &egui::Response,
    view: &mut TimelineView,
    layout: &TimelineLayout,
) {
    // 用指针位置判断是否在时间线区域内（不依赖 response.hovered()，
    // 因为 Clip 的 interact 区域会吞掉 hover 状态导致无法滚动）
    let pointer_in_rect = response.rect.contains(
        ui.input(|i| i.pointer.hover_pos())
            .unwrap_or(egui::pos2(-1.0, -1.0)),
    );
    if !pointer_in_rect {
        return;
    }

    let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
    let scroll_x = ui.input(|i| i.smooth_scroll_delta.x);
    let zoom_delta = ui.input(|i| i.zoom_delta());

    // 垂直滚动：上下移动轨道
    if scroll_y != 0.0 {
        view.scroll_y = (view.scroll_y - scroll_y).max(0.0);
    }

    // 水平滚动：左右移动时间轴
    if scroll_x != 0.0 {
        view.pan_by_pixels(scroll_x);
    }

    // 双指缩放 / Ctrl+滚轮缩放
    if zoom_delta != 1.0 {
        if let Some(mouse_pos) = response.hover_pos() {
            view.zoom_around_pointer(&layout.timeline_rect, mouse_pos.x, zoom_delta);
        }
    }
}
