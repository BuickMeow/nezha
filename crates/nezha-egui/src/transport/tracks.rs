use crate::transport::controller::TimelineCommand;
use crate::transport::hit_test::clip_hit_areas;
use crate::transport::layout::{TimelineLayout, TimelineMetrics};
use crate::transport::timecode::font;
use crate::transport::{
    ClipDragMode, ClipDragState, ClipKind, ThemeColors, TimelineDrawContext, TimelineView, Track,
    TrackKind,
};
use eframe::egui;

pub fn draw_tracks(ctx: &mut TimelineDrawContext<'_>, painter: &egui::Painter) -> (f32, bool) {
    let mut clip_clicked = false;
    let has_video = ctx
        .state
        .data
        .tracks
        .iter()
        .any(|track| track.kind == TrackKind::Video);
    let has_audio = ctx
        .state
        .data
        .tracks
        .iter()
        .any(|track| track.kind == TrackKind::Audio);

    // 确定被拖动 Clip 的类型，用于正确计算目标轨道索引
    let dragged_clip_kind = ctx.state.interaction.clip_drag.and_then(|drag| {
        ctx.state.data.tracks.iter()
            .flat_map(|t| t.clips.iter())
            .find(|c| c.id == drag.clip_id)
            .map(|c| c.kind)
    });

    let mut y = ctx.layout.ruler_rect.max.y - ctx.state.view.scroll_y;
    let view = &ctx.state.view;
    let selected_id = ctx.state.selection.selected_clip_id;
    let tracks = &ctx.state.data.tracks;
    let fps = ctx.fps;

    // 绘制视频轨道时，只计算视频轨道索引
    let mut video_track_index = 0usize;
    if has_video {
        for (track_index, track) in tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| track.kind == TrackKind::Video)
        {
            let (new_y, row_clicked) = draw_track_row(
                ctx.ui,
                painter,
                ctx.c,
                ctx.layout,
                ctx.metrics,
                view,
                selected_id,
                track,
                y,
                &ctx.state.interaction.clip_drag,
                ctx.commands,
                track_index,
                video_track_index,
                TrackKind::Video,
                dragged_clip_kind,
                fps,
            );
            y = new_y;
            video_track_index += 1;
            clip_clicked = clip_clicked || row_clicked;
        }
    }

    // 绘制音频轨道时，只计算音频轨道索引
    let mut audio_track_index = 0usize;
    if has_audio {
        for (track_index, track) in tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| track.kind == TrackKind::Audio)
        {
            let (new_y, row_clicked) = draw_track_row(
                ctx.ui,
                painter,
                ctx.c,
                ctx.layout,
                ctx.metrics,
                view,
                selected_id,
                track,
                y,
                &ctx.state.interaction.clip_drag,
                ctx.commands,
                track_index,
                audio_track_index,
                TrackKind::Audio,
                dragged_clip_kind,
                fps,
            );
            y = new_y;
            audio_track_index += 1;
            clip_clicked = clip_clicked || row_clicked;
        }
    }

    (y, clip_clicked)
}

fn draw_lock_button(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    header_rect: egui::Rect,
    track: &Track,
    track_index: usize,
    btn_size: f32,
    commands: &mut Vec<TimelineCommand>,
) {
    let lock_rect = egui::Rect::from_center_size(
        egui::pos2(header_rect.min.x + 36.0, header_rect.center().y),
        egui::vec2(btn_size, btn_size),
    );
    let lock_color = if track.locked {
        c.btn_locked_on
    } else {
        c.btn_locked_off
    };
    painter.rect_filled(lock_rect, 2.0, lock_color);
    painter.text(
        lock_rect.center(),
        egui::Align2::CENTER_CENTER,
        "\u{1f512}",
        font(9.0),
        egui::Color32::WHITE,
    );
    let lock_resp = ui.interact(
        lock_rect,
        egui::Id::new(("track_lock", track_index)),
        egui::Sense::click(),
    );
    if lock_resp.clicked() {
        commands.push(TimelineCommand::ToggleTrackLocked(track_index));
    }
}

fn draw_track_header_controls(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    header_rect: egui::Rect,
    track: &Track,
    track_index: usize,
    commands: &mut Vec<TimelineCommand>,
) {
    let btn_size = 16.0;

    if track.kind == TrackKind::Video {
        // 👁 可见性按钮
        let eye_rect = egui::Rect::from_center_size(
            egui::pos2(header_rect.min.x + 16.0, header_rect.center().y),
            egui::vec2(btn_size, btn_size),
        );
        let eye_color = if track.hidden {
            c.btn_hidden_on
        } else {
            c.btn_hidden_off
        };
        painter.rect_filled(eye_rect, 2.0, eye_color);
        painter.text(
            eye_rect.center(),
            egui::Align2::CENTER_CENTER,
            "\u{1f441}",
            font(9.0),
            egui::Color32::WHITE,
        );
        let eye_resp = ui.interact(
            eye_rect,
            egui::Id::new(("track_eye", track_index)),
            egui::Sense::click(),
        );
        if eye_resp.clicked() {
            commands.push(TimelineCommand::ToggleTrackHidden(track_index));
        }

        draw_lock_button(ui, painter, c, header_rect, track, track_index, btn_size, commands);

        painter.text(
            egui::pos2(header_rect.min.x + 52.0, header_rect.center().y),
            egui::Align2::LEFT_CENTER,
            &track.name,
            font(11.0),
            if track.hidden { c.dim_text } else { c.text },
        );
    } else {
        // 🔊 静音按钮
        let mute_rect = egui::Rect::from_center_size(
            egui::pos2(header_rect.min.x + 16.0, header_rect.center().y),
            egui::vec2(btn_size, btn_size),
        );
        let mute_color = if track.muted {
            c.btn_mute_on
        } else {
            c.btn_mute_off
        };
        painter.rect_filled(mute_rect, 2.0, mute_color);
        painter.text(
            mute_rect.center(),
            egui::Align2::CENTER_CENTER,
            "\u{1f50a}",
            font(9.0),
            egui::Color32::WHITE,
        );
        let mute_resp = ui.interact(
            mute_rect,
            egui::Id::new(("track_mute", track_index)),
            egui::Sense::click(),
        );
        if mute_resp.clicked() {
            commands.push(TimelineCommand::ToggleTrackMute(track_index));
        }

        draw_lock_button(ui, painter, c, header_rect, track, track_index, btn_size, commands);

        painter.text(
            egui::pos2(header_rect.min.x + 52.0, header_rect.center().y),
            egui::Align2::LEFT_CENTER,
            &track.name,
            font(11.0),
            if track.muted { c.dim_text } else { c.text },
        );
    }
}

/// Draw a single clip rectangle with selection highlight and label.
/// 返回深化的颜色，用于缓冲区显示。
fn darkened_color(color: egui::Color32) -> egui::Color32 {
    // 将 RGB 各通道压暗 ~20%
    egui::Color32::from_rgb(
        (color.r() as f32 * 0.78) as u8,
        (color.g() as f32 * 0.78) as u8,
        (color.b() as f32 * 0.78) as u8,
    )
}

/// 绘制 clip 视觉，如果提供了 content_rect 则绘制三段色。
/// content_start_x / content_end_x 是内容区域的左右 x 边界（clip_rect 坐标系内）。
#[allow(clippy::too_many_arguments)]
fn draw_clip_visual(
    painter: &egui::Painter,
    metrics: &TimelineMetrics,
    clip_rect: egui::Rect,
    hit_areas: &super::hit_test::ClipHitAreas,
    clip_color: egui::Color32,
    clip_name: &str,
    is_selected: bool,
    content_start_x: Option<f32>,
    content_end_x: Option<f32>,
) {
    // ── 三段式绘制 ──
    if let (Some(csx), Some(cex)) = (content_start_x, content_end_x) {
        let csx = csx.clamp(clip_rect.min.x, clip_rect.max.x);
        let cex = cex.clamp(clip_rect.min.x, clip_rect.max.x);

        if cex > csx && (csx > clip_rect.min.x || cex < clip_rect.max.x) {
            // 左缓冲区：clip_rect.min.x .. csx
            if csx > clip_rect.min.x {
                let left_rect = egui::Rect::from_min_max(
                    egui::pos2(clip_rect.min.x, clip_rect.min.y),
                    egui::pos2(csx, clip_rect.max.y),
                );
                painter.rect_filled(left_rect, 3.0, darkened_color(clip_color));
            }

            // 内容区：csx .. cex
            let content_rect = egui::Rect::from_min_max(
                egui::pos2(csx, clip_rect.min.y),
                egui::pos2(cex, clip_rect.max.y),
            );
            painter.rect_filled(content_rect, 3.0, clip_color);

            // 右缓冲区：cex .. clip_rect.max.x
            if cex < clip_rect.max.x {
                let right_rect = egui::Rect::from_min_max(
                    egui::pos2(cex, clip_rect.min.y),
                    egui::pos2(clip_rect.max.x, clip_rect.max.y),
                );
                painter.rect_filled(right_rect, 3.0, darkened_color(clip_color));
            }

            // 选中边框覆盖整个 clip
            if is_selected {
                painter.rect_stroke(
                    clip_rect,
                    3.0,
                    egui::Stroke::new(2.0, egui::Color32::WHITE),
                    egui::StrokeKind::Inside,
                );
                painter.rect_filled(
                    hit_areas.left_edge,
                    0.0,
                    egui::Color32::from_white_alpha(60),
                );
                painter.rect_filled(
                    hit_areas.right_edge,
                    0.0,
                    egui::Color32::from_white_alpha(60),
                );
            }

            // 标签放在内容区左侧
            if clip_rect.width() > metrics.clip_label_min_width {
                let label_x =
                    csx.max(clip_rect.min.x + metrics.clip_edge_width + metrics.clip_text_padding);
                painter.text(
                    egui::pos2(label_x, clip_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    clip_name,
                    font(10.0),
                    egui::Color32::WHITE,
                );
            }
            return;
        }
    }

    // ── 无偏移，原始单色绘制 ──
    painter.rect_filled(clip_rect, 3.0, clip_color);

    if is_selected {
        painter.rect_stroke(
            clip_rect,
            3.0,
            egui::Stroke::new(2.0, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
        painter.rect_filled(
            hit_areas.left_edge,
            0.0,
            egui::Color32::from_white_alpha(60),
        );
        painter.rect_filled(
            hit_areas.right_edge,
            0.0,
            egui::Color32::from_white_alpha(60),
        );
    }

    if clip_rect.width() > metrics.clip_label_min_width {
        painter.text(
            egui::pos2(
                clip_rect.min.x + metrics.clip_edge_width + metrics.clip_text_padding,
                clip_rect.center().y,
            ),
            egui::Align2::LEFT_CENTER,
            clip_name,
            font(10.0),
            egui::Color32::WHITE,
        );
    }
}

/// Check clip type vs target track type and issue MoveClipToTrack commands when dragging.
fn handle_drag_to_track(
    ui: &egui::Ui,
    track_rect: egui::Rect,
    track_kind_index: usize,
    track_kind: TrackKind,
    clip_kind: ClipKind,
    clip_id: usize,
    commands: &mut Vec<TimelineCommand>,
) {
    let clip_belongs_to_video = clip_kind.is_video_track_kind();
    let target_is_video = track_kind == TrackKind::Video;
    let clip_target_kind = if clip_belongs_to_video {
        TrackKind::Video
    } else {
        TrackKind::Audio
    };
    if clip_belongs_to_video == target_is_video {
        if let Some(ptr) = ui.input(|i| i.pointer.hover_pos()) {
            if ptr.y < track_rect.min.y {
                let target = if track_kind_index > 0 { track_kind_index - 1 } else { 0 };
                commands.push(TimelineCommand::MoveClipToTrack {
                    clip_id,
                    target_track_index: target,
                    target_track_kind: track_kind,
                });
            } else if ptr.y > track_rect.max.y {
                commands.push(TimelineCommand::MoveClipToTrack {
                    clip_id,
                    target_track_index: track_kind_index + 1,
                    target_track_kind: track_kind,
                });
            }
        }
    } else {
        if let Some(ptr) = ui.input(|i| i.pointer.hover_pos()) {
            if ptr.y < track_rect.min.y && track_kind_index == 0 {
                commands.push(TimelineCommand::MoveClipToTrack {
                    clip_id,
                    target_track_index: 0,
                    target_track_kind: clip_target_kind,
                });
            } else if ptr.y > track_rect.max.y {
                commands.push(TimelineCommand::MoveClipToTrack {
                    clip_id,
                    target_track_index: usize::MAX,
                    target_track_kind: clip_target_kind,
                });
            }
        }
    }
}

/// Handle drag interactions for a selected clip (resize left/right, move).
#[allow(clippy::too_many_arguments)]
fn handle_selected_clip_interaction(
    ui: &mut egui::Ui,
    layout: &TimelineLayout,
    view: &TimelineView,
    track_rect: egui::Rect,
    _track_index: usize,
    track_kind_index: usize,
    track_kind: TrackKind,
    clip_kind: ClipKind,
    clip_id: usize,
    clip_start: f32,
    clip_end: f32,
    hit_areas: &super::hit_test::ClipHitAreas,
    clip_rect: egui::Rect,
    active_clip_drag: &mut Option<ClipDragState>,
    commands: &mut Vec<TimelineCommand>,
    clip_clicked: &mut bool,
    dragged_clip_id: &mut Option<usize>,
) {
    let left_interact = ui
        .interact(
            hit_areas.left_edge,
            egui::Id::new(("clip_left", clip_id)),
            egui::Sense::drag(),
        )
        .on_hover_cursor(egui::CursorIcon::ResizeWest);
    if left_interact.drag_started() {
        let pointer_time = view.time_at_screen_x(
            &layout.timeline_rect,
            left_interact
                .interact_pointer_pos()
                .map(|pos| pos.x)
                .unwrap_or(clip_rect.min.x),
        );
        let drag_state = ClipDragState {
            clip_id,
            mode: ClipDragMode::ResizeStart,
            anchor_pointer_time: pointer_time,
            anchor_start: clip_start,
            anchor_end: clip_end,
            track_was_inserted: false,
        };
        *active_clip_drag = Some(drag_state);
        commands.push(TimelineCommand::SetClipDrag(Some(drag_state)));
    }
    if left_interact.dragged() {
        if let (Some(drag), Some(pointer_pos)) = (
            active_clip_drag.as_ref(),
            left_interact.interact_pointer_pos(),
        ) && drag.clip_id == clip_id
            && drag.mode == ClipDragMode::ResizeStart
        {
            let pointer_time = view.time_at_screen_x(&layout.timeline_rect, pointer_pos.x);
            let new_start = drag.anchor_start + (pointer_time - drag.anchor_pointer_time);
            commands.push(TimelineCommand::ResizeClipStartTo {
                clip_id,
                start: new_start,
            });
        }
        commands.push(TimelineCommand::SelectClip(clip_id));
        *dragged_clip_id = Some(clip_id);
    }
    let right_interact = ui
        .interact(
            hit_areas.right_edge,
            egui::Id::new(("clip_right", clip_id)),
            egui::Sense::drag(),
        )
        .on_hover_cursor(egui::CursorIcon::ResizeEast);
    if right_interact.drag_started() {
        let pointer_time = view.time_at_screen_x(
            &layout.timeline_rect,
            right_interact
                .interact_pointer_pos()
                .map(|pos| pos.x)
                .unwrap_or(clip_rect.max.x),
        );
        let drag_state = ClipDragState {
            clip_id,
            mode: ClipDragMode::ResizeEnd,
            anchor_pointer_time: pointer_time,
            anchor_start: clip_start,
            anchor_end: clip_end,
            track_was_inserted: false,
        };
        *active_clip_drag = Some(drag_state);
        commands.push(TimelineCommand::SetClipDrag(Some(drag_state)));
    }
    if right_interact.dragged() {
        if let (Some(drag), Some(pointer_pos)) = (
            active_clip_drag.as_ref(),
            right_interact.interact_pointer_pos(),
        ) && drag.clip_id == clip_id
            && drag.mode == ClipDragMode::ResizeEnd
        {
            let pointer_time = view.time_at_screen_x(&layout.timeline_rect, pointer_pos.x);
            let new_end = drag.anchor_end + (pointer_time - drag.anchor_pointer_time);
            commands.push(TimelineCommand::ResizeClipEndTo {
                clip_id,
                end: new_end,
            });
        }
        commands.push(TimelineCommand::SelectClip(clip_id));
        *dragged_clip_id = Some(clip_id);
    }
    // Only create move interact if there's space between resize edges.
    if hit_areas.middle_rect.width() > 0.0 {
        let mid_interact = ui
            .interact(
                hit_areas.middle_rect,
                egui::Id::new(("clip_mid", clip_id)),
                egui::Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::Grab);
        if mid_interact.clicked() {
            commands.push(TimelineCommand::SelectClip(clip_id));
            *clip_clicked = true;
        }
        if mid_interact.drag_started() {
            let pointer_time = view.time_at_screen_x(
                &layout.timeline_rect,
                mid_interact
                    .interact_pointer_pos()
                    .map(|pos| pos.x)
                    .unwrap_or(clip_rect.center().x),
            );
            let drag_state = ClipDragState {
                clip_id,
                mode: ClipDragMode::Move,
                anchor_pointer_time: pointer_time,
                anchor_start: clip_start,
                anchor_end: clip_end,
                track_was_inserted: false,
            };
            *active_clip_drag = Some(drag_state);
            commands.push(TimelineCommand::SetClipDrag(Some(drag_state)));
        }
        if mid_interact.dragged() {
            if let (Some(drag), Some(pointer_pos)) = (
                active_clip_drag.as_ref(),
                mid_interact.interact_pointer_pos(),
            ) && drag.clip_id == clip_id
                && drag.mode == ClipDragMode::Move
            {
                let pointer_time = view.time_at_screen_x(&layout.timeline_rect, pointer_pos.x);
                let new_start = drag.anchor_start + (pointer_time - drag.anchor_pointer_time);
                commands.push(TimelineCommand::MoveClipToStart {
                    clip_id,
                    start: new_start,
                });
            }
            commands.push(TimelineCommand::SelectClip(clip_id));
            *dragged_clip_id = Some(clip_id);
            handle_drag_to_track(
                ui,
                track_rect,
                track_kind_index,
                track_kind,
                clip_kind,
                clip_id,
                commands,
            );
        }
    }
}

/// Handle drag interactions for an unselected clip (move only).
#[allow(clippy::too_many_arguments)]
fn handle_unselected_clip_interaction(
    ui: &mut egui::Ui,
    layout: &TimelineLayout,
    view: &TimelineView,
    track_rect: egui::Rect,
    _track_index: usize,
    track_kind_index: usize,
    track_kind: TrackKind,
    clip_kind: ClipKind,
    clip_id: usize,
    clip_start: f32,
    clip_end: f32,
    clip_rect: egui::Rect,
    active_clip_drag: &mut Option<ClipDragState>,
    commands: &mut Vec<TimelineCommand>,
    clip_clicked: &mut bool,
    dragged_clip_id: &mut Option<usize>,
) {
    let clip_interact = ui
        .interact(
            clip_rect,
            egui::Id::new(("timeline_clip", clip_id)),
            egui::Sense::drag(),
        )
        .on_hover_cursor(egui::CursorIcon::Grab);
    if clip_interact.clicked() {
        commands.push(TimelineCommand::SelectClip(clip_id));
        *clip_clicked = true;
    }
    if clip_interact.drag_started() {
        let pointer_time = view.time_at_screen_x(
            &layout.timeline_rect,
            clip_interact
                .interact_pointer_pos()
                .map(|pos| pos.x)
                .unwrap_or(clip_rect.center().x),
        );
        let drag_state = ClipDragState {
            clip_id,
            mode: ClipDragMode::Move,
            anchor_pointer_time: pointer_time,
            anchor_start: clip_start,
            anchor_end: clip_end,
            track_was_inserted: false,
        };
        *active_clip_drag = Some(drag_state);
        commands.push(TimelineCommand::SetClipDrag(Some(drag_state)));
    }
    if clip_interact.dragged() {
        if let (Some(drag), Some(pointer_pos)) = (
            active_clip_drag.as_ref(),
            clip_interact.interact_pointer_pos(),
        ) && drag.clip_id == clip_id
            && drag.mode == ClipDragMode::Move
        {
            let pointer_time = view.time_at_screen_x(&layout.timeline_rect, pointer_pos.x);
            let new_start = drag.anchor_start + (pointer_time - drag.anchor_pointer_time);
            commands.push(TimelineCommand::MoveClipToStart {
                clip_id,
                start: new_start,
            });
        }
        commands.push(TimelineCommand::SelectClip(clip_id));
        *dragged_clip_id = Some(clip_id);
        handle_drag_to_track(
            ui,
            track_rect,
            track_kind_index,
            track_kind,
            clip_kind,
            clip_id,
            commands,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_track_row(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    layout: &TimelineLayout,
    metrics: &TimelineMetrics,
    view: &TimelineView,
    selected_id: Option<usize>,
    track: &Track,
    y: f32,
    clip_drag: &Option<ClipDragState>,
    commands: &mut Vec<TimelineCommand>,
    track_index: usize,
    track_kind_index: usize,
    track_kind: TrackKind,
    _dragged_clip_kind: Option<ClipKind>,
    fps: u32,
) -> (f32, bool) {
    let mut clip_clicked = false;
    let visible_start = layout.visible_start;
    let visible_end = layout.visible_end;
    let track_bg = match track.kind {
        TrackKind::Video => c.video_track_bg,
        TrackKind::Audio => c.audio_track_bg,
    };

    let track_rect = layout.track_rect(y, view.track_height);
    painter.rect_filled(track_rect, 0.0, track_bg);
    painter.rect_stroke(
        track_rect,
        0.0,
        egui::Stroke::new(1.0, c.border),
        egui::StrokeKind::Inside,
    );

    let header_rect = layout.header_rect(&track_rect, view.header_width);
    let header_color = if track.muted {
        c.header_bg_muted
    } else {
        c.header_bg
    };
    painter.rect_filled(header_rect, 0.0, header_color);
    painter.rect_stroke(
        header_rect,
        0.0,
        egui::Stroke::new(1.0, c.border),
        egui::StrokeKind::Inside,
    );

    draw_track_header_controls(ui, painter, c, header_rect, track, track_index, commands);

    let mut dragged_clip_id = None;
    let primary_dragging = ui.input(|i| i.pointer.primary_down());
    let mut active_clip_drag = *clip_drag;
    if !primary_dragging {
        active_clip_drag = None;
        commands.push(TimelineCommand::SetClipDrag(None));
    }

    for clip_idx in 0..track.clips.len() {
        let clip_start = track.clips[clip_idx].start;
        let clip_end = track.clips[clip_idx].end;
        if clip_end < visible_start || clip_start > visible_end {
            continue;
        }
        let clip_id = track.clips[clip_idx].id;
        let clip_name = &track.clips[clip_idx].name;
        let clip_color = track.clips[clip_idx].color;
        let clip_kind = track.clips[clip_idx].kind;
        let hit_areas = clip_hit_areas(layout, metrics, view, &track_rect, clip_start, clip_end);
        let clip_rect = hit_areas.clip_rect;
        if clip_rect.width() > 0.0 {
            let is_selected = selected_id == Some(clip_id);
            if !track.locked {
                if is_selected {
                    handle_selected_clip_interaction(
                        ui,
                        layout,
                        view,
                        track_rect,
                        track_index,
                        track_kind_index,
                        track_kind,
                        clip_kind,
                        clip_id,
                        clip_start,
                        clip_end,
                        &hit_areas,
                        clip_rect,
                        &mut active_clip_drag,
                        commands,
                        &mut clip_clicked,
                        &mut dragged_clip_id,
                    );
                } else {
                    handle_unselected_clip_interaction(
                        ui,
                        layout,
                        view,
                        track_rect,
                        track_index,
                        track_kind_index,
                        track_kind,
                        clip_kind,
                        clip_id,
                        clip_start,
                        clip_end,
                        clip_rect,
                        &mut active_clip_drag,
                        commands,
                        &mut clip_clicked,
                        &mut dragged_clip_id,
                    );
                }
            } else {
                let clip_interact = ui.interact(
                    clip_rect,
                    egui::Id::new(("timeline_clip", clip_id)),
                    egui::Sense::click(),
                );
                if clip_interact.clicked() {
                    commands.push(TimelineCommand::SelectClip(clip_id));
                    clip_clicked = true;
                }
            }

            // 只对 Waterfall clip 计算三段内容区域边界（像素 x 坐标）
            let (content_start_x, content_end_x) =
                if track.clips[clip_idx].kind == ClipKind::Waterfall {
                    let clip = &track.clips[clip_idx];
                    if clip.content_start_offset > 0 || clip.content_end_offset > 0 {
                        let cs_time = clip.content_start_time(fps);
                        let ce_time = clip.content_end_time(fps);
                        let csx = view.screen_x_for_time(&layout.timeline_rect, cs_time);
                        let cex = view.screen_x_for_time(&layout.timeline_rect, ce_time);
                        (Some(csx), Some(cex))
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };

            draw_clip_visual(
                painter,
                metrics,
                clip_rect,
                &hit_areas,
                clip_color,
                &clip_name,
                is_selected,
                content_start_x,
                content_end_x,
            );
        }
    }

    if !clip_clicked && dragged_clip_id.is_none() {
        let deselect_area = egui::Rect::from_min_max(
            egui::pos2(track_rect.min.x, track_rect.max.y - 4.0),
            egui::pos2(track_rect.max.x, track_rect.max.y),
        );
        let deselect = ui.interact(
            deselect_area,
            egui::Id::new(("track_deselect", &track.name)),
            egui::Sense::click(),
        );
        if deselect.clicked() {
            commands.push(TimelineCommand::ClearSelection);
        }
    }

    // Set cursor based on active clip drag
    if let Some(drag) = active_clip_drag {
        match drag.mode {
            ClipDragMode::Move => {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
            ClipDragMode::ResizeStart => {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeWest);
            }
            ClipDragMode::ResizeEnd => {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeEast);
            }
        }
    }

    (y + view.track_height, clip_clicked)
}
