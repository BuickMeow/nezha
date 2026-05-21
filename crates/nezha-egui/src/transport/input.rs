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

    let mouse_pos = ui.input(|i| i.pointer.hover_pos());
    let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
    let scroll_x = ui.input(|i| i.smooth_scroll_delta.x);
    let zoom_delta = ui.input(|i| i.zoom_delta());

    if scroll_y != 0.0 {
        if let Some(pos) = mouse_pos {
            if layout.scrollbar_rect.contains(pos) {
                // 滚动条上滚动 → 缩放（以指针位置为中心）
                view.zoom_around_pointer(&layout.timeline_rect, pos.x, 1.0 + scroll_y * 0.003);
            } else if layout.ruler_rect.contains(pos) {
                // 标尺上滚动 → 左右移动时间线
                view.pan_by_pixels(scroll_y * 2.0);
            } else {
                // 轨道上滚动 → 上下滚动轨道
                view.scroll_y = (view.scroll_y - scroll_y).max(0.0);
            }
        } else {
            view.scroll_y = (view.scroll_y - scroll_y).max(0.0);
        }
    }

    // 水平滚动：左右移动时间轴
    if scroll_x != 0.0 {
        view.pan_by_pixels(scroll_x);
    }

    // 双指缩放 / Ctrl+滚轮缩放
    if zoom_delta != 1.0
        && let Some(mouse_pos) = mouse_pos
    {
        view.zoom_around_pointer(&layout.timeline_rect, mouse_pos.x, zoom_delta);
    }

    // 按住中键自由拖拽背景移动
    if ui.input(|i| i.pointer.button_down(egui::PointerButton::Middle)) {
        let delta = ui.input(|i| i.pointer.delta());
        view.scroll_offset -= delta.x / view.zoom;
        view.scroll_y = (view.scroll_y - delta.y).max(0.0);
        view.clamp_scroll();
    }
}
