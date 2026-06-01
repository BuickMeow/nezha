use std::collections::HashSet;

use eframe::egui;

// ── Shared draw context ──

/// Common parameters passed to timeline draw functions.
pub(crate) struct TimelineDrawContext<'a> {
    pub ui: &'a mut egui::Ui,
    pub painter: &'a egui::Painter,
    pub c: &'a ThemeColors,
    pub layout: &'a TimelineLayout,
    pub metrics: &'a TimelineMetrics,
    pub state: &'a TimelineState,
    pub response: &'a egui::Response,
    pub duration: f32,
    pub fps: u32,
    pub current_time: f32,
    pub commands: &'a mut Vec<TimelineCommand>,
}

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
mod snap;
mod theme;
mod timecode;
mod tracks;
mod types;
mod view;

pub use data::{next_audio_track_name, next_video_track_name};
pub use interaction::{ClipDragMode, ClipDragState, ScrollbarDrag};
pub use model::TimelineState;
pub use nezha_compositor::BlendMode;
pub use theme::ThemeColors;
pub use types::{ClipKind, LayerCommon, Track, TrackClip, TrackKind};
pub use view::TimelineView;

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
        .count() as f32
        * state.view.track_height;
    state
        .view
        .clamp_scroll_y(track_area_height, total_track_height);

    let layout = TimelineLayout::new(timeline_rect, &state.view, &metrics);

    let track_painter = ui.painter_at(egui::Rect::from_min_max(
        egui::pos2(timeline_rect.min.x, layout.ruler_rect.max.y),
        egui::pos2(timeline_rect.max.x, layout.content_bottom),
    ));

    let mut ctx = TimelineDrawContext {
        ui,
        painter: &painter,
        c: &c,
        layout: &layout,
        metrics: &metrics,
        state,
        response: &response,
        duration,
        fps,
        current_time: *current_time,
        commands: &mut commands,
    };

    draw_scrollbar(&mut ctx);

    let (y, clip_clicked) = draw_tracks(&mut ctx, &track_painter);

    let clear_selection = response.clicked() && !clip_clicked;

    draw_ruler(&mut ctx, *current_time);

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

    draw_playhead(&mut ctx, *current_time);

    draw_controls(&mut ctx, *is_playing, *current_time);

    if clear_selection {
        ctx.commands.push(TimelineCommand::ClearSelection);
    }

    // 绘制吸附指示线
    if let Some(snap_time) = state.interaction.snap_line {
        let x = state.view.screen_x_for_time(&timeline_rect, snap_time);
        if x >= timeline_rect.min.x + state.view.header_width {
            painter.line_segment(
                [
                    egui::pos2(x, layout.ruler_rect.max.y),
                    egui::pos2(x, layout.content_bottom),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 200, 255)),
            );
        }
    }

    // 绘制框选矩形
    if let Some(bs) = &state.interaction.box_select {
        let sel_rect = egui::Rect::from_two_pos(
            egui::pos2(bs.start_x, bs.start_y),
            egui::pos2(bs.current_x, bs.current_y),
        );
        painter.rect_filled(sel_rect, 0.0, egui::Color32::from_rgba_premultiplied(0, 120, 255, 40));
        painter.rect_stroke(
            sel_rect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 120, 255)),
            egui::StrokeKind::Inside,
        );
    }

    // ── 框选交互 ──
    if response.drag_started()
        && !clip_clicked
        && let Some(pos) = response.interact_pointer_pos()
        && pos.y >= layout.ruler_rect.max.y
        && pos.y <= layout.content_bottom
    {
        let mode = if ui.input(|i| i.modifiers.ctrl || i.modifiers.command) {
            interaction::BoxSelectMode::Toggle
        } else if ui.input(|i| i.modifiers.shift) {
            interaction::BoxSelectMode::Add
        } else {
            interaction::BoxSelectMode::Replace
        };
        commands.push(TimelineCommand::StartBoxSelect(pos.x, pos.y, mode));
    }

    if state.interaction.box_select.is_some() {
        if response.dragged()
            && let Some(pos) = response.interact_pointer_pos()
        {
            commands.push(TimelineCommand::UpdateBoxSelect(pos.x, pos.y));
        }
        if response.drag_stopped()
            && let Some(bs) = &state.interaction.box_select
        {
            let sel_rect = egui::Rect::from_two_pos(
                egui::pos2(bs.start_x, bs.start_y),
                egui::pos2(bs.current_x, bs.current_y),
            );
            let mut hit_ids = HashSet::new();
            for track in &state.data.tracks {
                for clip in &track.clips {
                    let clip_start_x = state.view.screen_x_for_time(&timeline_rect, clip.start);
                    let clip_end_x = state.view.screen_x_for_time(&timeline_rect, clip.end);
                    let clip_rect = egui::Rect::from_min_max(
                        egui::pos2(clip_start_x, layout.ruler_rect.max.y),
                        egui::pos2(clip_end_x, layout.content_bottom),
                    );
                    if sel_rect.intersects(clip_rect) {
                        hit_ids.insert(clip.id);
                    }
                }
            }
            commands.push(TimelineCommand::FinishBoxSelect(hit_ids));
        }
    }

    apply_timeline_commands(is_playing, current_time, state, commands);

    // 清除吸附线状态（每帧重置）
    state.interaction.snap_line = None;
}
