use eframe::egui;

// ── 子模块 ──

mod controller;
mod controls;
mod input;
mod model;
mod playhead;
mod ruler;
mod scrollbar;
mod theme;
mod timecode;
mod tracks;

pub use model::{
    ClipKind, ScrollbarDrag, TimelineInteraction, TimelineState, TimelineView, Track, TrackClip,
    TrackKind,
};
pub use theme::ThemeColors;

use controls::draw_controls;
use input::handle_input;
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

    let available = ui.available_size();
    let response = ui.allocate_response(available, egui::Sense::click_and_drag());
    let rect = response.rect;
    let painter = ui.painter_at(rect);

    let ruler_height = 26.0;
    let scrollbar_height = 16.0;
    let controls_height = 32.0;

    let content_width = (rect.width() - state.view.header_width).max(1.0);
    let timeline_rect = rect;
    let fps = state.fps;

    // ── 输入处理 ──
    handle_input(ui, &response, &mut state.view, &timeline_rect);

    let (visible_start, visible_end) = state.view.visible_range(content_width);

    // ── 标尺 ──
    draw_ruler(
        ui,
        &painter,
        &c,
        &timeline_rect,
        state,
        visible_start,
        visible_end,
        ruler_height,
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
        &timeline_rect,
        &mut state.view,
        &mut state.interaction,
        duration,
        content_width,
        scrollbar_height,
        controls_height,
        &response,
        fps,
    );

    // ── 轨道 ──
    let y = draw_tracks(
        ui,
        &painter,
        &c,
        &timeline_rect,
        state,
        visible_start,
        visible_end,
        ruler_height,
        scrollbar_height,
        controls_height,
    );

    // 底部填充
    let content_bottom = timeline_rect.max.y - controls_height - scrollbar_height;
    if y < content_bottom {
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(timeline_rect.min.x, y),
                egui::pos2(timeline_rect.max.x, content_bottom),
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
        &timeline_rect,
        &response,
        state,
        current_time,
        duration,
        ruler_height,
        controls_height,
        scrollbar_height,
        fps,
    );

    // ── 底部控制栏 ──
    draw_controls(
        ui,
        &painter,
        &c,
        &timeline_rect,
        is_playing,
        current_time,
        duration,
        state,
        controls_height,
    );
}
