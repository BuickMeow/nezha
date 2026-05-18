use eframe::egui;

// ── 子模块 ──

mod controller;
mod controls;
mod hit_test;
mod input;
mod layout;
mod model;
mod playhead;
mod ruler;
mod scrollbar;
mod theme;
mod timecode;
mod tracks;

pub use model::{
    ClipDragMode, ClipDragState, ClipKind, ScrollbarDrag, TimelineInteraction, TimelineState,
    TimelineView, Track, TrackClip, TrackKind,
};
pub use theme::ThemeColors;

use controls::draw_controls;
use input::handle_input;
use layout::{TimelineLayout, TimelineMetrics};
use playhead::draw_playhead;
use ruler::draw_ruler;
use scrollbar::draw_scrollbar;
use tracks::draw_tracks;

pub fn show(
    ui: &mut egui::Ui,
    is_playing: &mut bool,
    current_time: &mut f32,
    duration: f32,
    state: &mut TimelineState,
    dark_mode: bool,
) {
    let c = ThemeColors::new(dark_mode);
    let metrics = TimelineMetrics::default();

    let available = ui.available_size();
    let response = ui.allocate_response(available, egui::Sense::click_and_drag());
    let rect = response.rect;
    let painter = ui.painter_at(rect);
    let timeline_rect = rect;
    let layout = TimelineLayout::new(timeline_rect, &state.view, &metrics);
    let fps = state.fps;

    // ── 输入处理 ──
    handle_input(ui, &response, &mut state.view, &layout);
    let layout = TimelineLayout::new(timeline_rect, &state.view, &metrics);

    // ── 标尺 ──
    draw_ruler(
        ui,
        &painter,
        &c,
        &layout,
        &metrics,
        state,
        &response,
        current_time,
        duration,
        fps,
    );

    // ── 滚动条 ──
    draw_scrollbar(
        ui,
        &painter,
        &c,
        &layout,
        &metrics,
        &mut state.view,
        &mut state.interaction,
        duration,
        &response,
        fps,
    );

    // ── 轨道 ──
    let y = draw_tracks(
        ui,
        &painter,
        &c,
        &layout,
        &metrics,
        state,
    );

    // 底部填充
    if y < layout.content_bottom {
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(timeline_rect.min.x, y),
                egui::pos2(timeline_rect.max.x, layout.content_bottom),
            ),
            0.0,
            c.bg,
        );
    }

    // ── 播放头 ──
    draw_playhead(
        ui,
        &painter,
        &c,
        &layout,
        &metrics,
        &response,
        state,
        current_time,
        duration,
        fps,
    );

    // ── 底部控制栏 ──
    draw_controls(
        ui,
        &painter,
        &c,
        &layout,
        is_playing,
        current_time,
        duration,
        state,
    );
}
