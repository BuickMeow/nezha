use eframe::egui;

// ── 子模块 ──

mod controller;
mod controls;
mod data;
mod hit_test;
mod input;
mod interaction;
mod layout;
mod model;
mod playhead;
mod ruler;
mod scrollbar;
mod selection;
mod theme;
mod timecode;
mod tracks;
mod types;
mod view;

pub use data::{next_audio_track_name, next_video_track_name};
pub use interaction::{ClipDragMode, ClipDragState, ScrollbarDrag, TimelineInteraction};
pub use model::TimelineState;
pub use theme::ThemeColors;
pub use types::{ClipKind, LayerCommon, Track, TrackClip, TrackKind};
pub use view::TimelineView;
pub use nezha_compositor::BlendMode;

use controller::{TimelineCommand, apply_timeline_commands};
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
    let mut commands = Vec::<TimelineCommand>::new();

    let available = ui.available_size();
    let response = ui.allocate_response(available, egui::Sense::click_and_drag());
    let rect = response.rect;
    let track_area_height =
        (rect.height() - metrics.ruler_height - metrics.controls_height - metrics.scrollbar_height)
            .max(1.0);
    let painter = ui.painter_at(rect);
    let timeline_rect = rect;
    let layout = TimelineLayout::new(timeline_rect, &state.view, &metrics);
    let fps = state.fps;

    handle_input(ui, &response, &mut state.view, &layout);

    let total_track_height = state
        .data
        .tracks
        .iter()
        .filter(|t| t.kind == TrackKind::Video)
        .count() as f32
        * state.view.track_height;
    state
        .view
        .clamp_scroll_y(track_area_height, total_track_height);

    let layout = TimelineLayout::new(timeline_rect, &state.view, &metrics);

    draw_scrollbar(
        ui,
        &painter,
        &c,
        &layout,
        &metrics,
        &state.view,
        &state.interaction,
        duration,
        &response,
        fps,
        &mut commands,
    );

    let track_painter = ui.painter_at(egui::Rect::from_min_max(
        egui::pos2(timeline_rect.min.x, layout.ruler_rect.max.y),
        egui::pos2(timeline_rect.max.x, layout.content_bottom),
    ));
    let (y, clip_clicked) = draw_tracks(
        ui,
        &track_painter,
        &c,
        &layout,
        &metrics,
        state,
        &mut commands,
    );

    if response.clicked() && !clip_clicked {
        commands.push(TimelineCommand::ClearSelection);
    }

    draw_ruler(
        ui,
        &painter,
        &c,
        &layout,
        &metrics,
        state,
        &response,
        *current_time,
        duration,
        fps,
        &mut commands,
    );

    if y < layout.content_bottom {
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(timeline_rect.min.x, y.max(layout.ruler_rect.max.y)),
                egui::pos2(timeline_rect.max.x, layout.content_bottom),
            ),
            0.0,
            c.bg,
        );
    }

    draw_playhead(
        ui,
        &painter,
        &c,
        &layout,
        &metrics,
        &response,
        state,
        *current_time,
        duration,
        fps,
        &mut commands,
    );

    draw_controls(
        ui,
        &painter,
        &c,
        &layout,
        *is_playing,
        *current_time,
        duration,
        state,
        &mut commands,
    );

    apply_timeline_commands(is_playing, current_time, state, commands);
}
