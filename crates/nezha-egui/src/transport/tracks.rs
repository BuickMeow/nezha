use crate::transport::controller::TimelineCommand;
use crate::transport::hit_test::clip_hit_areas;
use crate::transport::layout::{TimelineLayout, TimelineMetrics};
use crate::transport::timecode::font;
use crate::transport::{
    ClipDragMode, ClipDragState, ThemeColors, TimelineState, TimelineView, Track, TrackKind,
};
use eframe::egui;

pub fn draw_tracks(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    layout: &TimelineLayout,
    metrics: &TimelineMetrics,
    state: &TimelineState,
    commands: &mut Vec<TimelineCommand>,
) -> f32 {
    let timeline_rect = layout.timeline_rect;
    let has_video = state
        .data
        .tracks
        .iter()
        .any(|track| track.kind == TrackKind::Video);
    let has_audio = state
        .data
        .tracks
        .iter()
        .any(|track| track.kind == TrackKind::Audio);

    let mut y = layout.ruler_rect.max.y;
    let view = &state.view;
    let selected_id = state.selected_clip_id;
    let tracks = &state.data.tracks;

    if has_video {
        let label_rect = layout.section_label_rect(y, metrics);
        painter.rect_filled(label_rect, 0.0, c.video_label_bg);
        painter.text(
            egui::pos2(
                timeline_rect.min.x + 8.0,
                y + metrics.section_label_height / 2.0,
            ),
            egui::Align2::LEFT_CENTER,
            "视频",
            font(11.0),
            c.dim_text,
        );
        y += metrics.section_label_height;

        for (track_index, track) in tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| track.kind == TrackKind::Video)
        {
            y = draw_track_row(
                ui,
                painter,
                c,
                layout,
                metrics,
                view,
                selected_id,
                track,
                y,
                &state.interaction.clip_drag,
                commands,
                track_index,
            );
        }
    }

    if has_audio {
        y += metrics.section_gap;
        let label_rect = layout.section_label_rect(y, metrics);
        painter.rect_filled(label_rect, 0.0, c.audio_label_bg);
        painter.text(
            egui::pos2(
                timeline_rect.min.x + 8.0,
                y + metrics.section_label_height / 2.0,
            ),
            egui::Align2::LEFT_CENTER,
            "音频",
            font(11.0),
            c.dim_text,
        );
        y += metrics.section_label_height;

        for (track_index, track) in tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| track.kind == TrackKind::Audio)
        {
            y = draw_track_row(
                ui,
                painter,
                c,
                layout,
                metrics,
                view,
                selected_id,
                track,
                y,
                &state.interaction.clip_drag,
                commands,
                track_index,
            );
        }
    }

    y
}

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
) -> f32 {
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

    if track.kind == TrackKind::Video {
        let btn_size = 16.0;
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
            "M",
            font(9.0),
            egui::Color32::WHITE,
        );

        let solo_rect = egui::Rect::from_center_size(
            egui::pos2(header_rect.min.x + 36.0, header_rect.center().y),
            egui::vec2(btn_size, btn_size),
        );
        let solo_color = if track.solo {
            c.btn_solo_on
        } else {
            c.btn_solo_off
        };
        painter.rect_filled(solo_rect, 2.0, solo_color);
        painter.text(
            solo_rect.center(),
            egui::Align2::CENTER_CENTER,
            "S",
            font(9.0),
            egui::Color32::WHITE,
        );

        painter.text(
            egui::pos2(header_rect.min.x + 52.0, header_rect.center().y),
            egui::Align2::LEFT_CENTER,
            &track.name,
            font(11.0),
            if track.muted { c.dim_text } else { c.text },
        );
    } else {
        painter.text(
            egui::pos2(header_rect.min.x + 8.0, header_rect.center().y),
            egui::Align2::LEFT_CENTER,
            &track.name,
            font(11.0),
            if track.muted { c.dim_text } else { c.text },
        );
    }

    let mut clip_clicked = false;
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
        let clip_name = track.clips[clip_idx].name.clone();
        let clip_color = track.clips[clip_idx].color;
        let hit_areas = clip_hit_areas(layout, metrics, view, &track_rect, clip_start, clip_end);
        let clip_rect = hit_areas.clip_rect;
        if clip_rect.width() > 0.0 {
            let is_selected = selected_id == Some(clip_id);

            if is_selected {
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
                    };
                    active_clip_drag = Some(drag_state);
                    commands.push(TimelineCommand::SetClipDrag(Some(drag_state)));
                }
                if left_interact.dragged() {
                    if let (Some(drag), Some(pointer_pos)) =
                        (active_clip_drag, left_interact.interact_pointer_pos())
                    {
                        if drag.clip_id == clip_id && drag.mode == ClipDragMode::ResizeStart {
                            let pointer_time =
                                view.time_at_screen_x(&layout.timeline_rect, pointer_pos.x);
                            let new_start =
                                drag.anchor_start + (pointer_time - drag.anchor_pointer_time);
                            commands.push(TimelineCommand::ResizeClipStartTo {
                                clip_id,
                                start: new_start,
                            });
                        }
                    }
                    commands.push(TimelineCommand::SelectClip(clip_id));
                    dragged_clip_id = Some(clip_id);
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
                    };
                    active_clip_drag = Some(drag_state);
                    commands.push(TimelineCommand::SetClipDrag(Some(drag_state)));
                }
                if right_interact.dragged() {
                    if let (Some(drag), Some(pointer_pos)) =
                        (active_clip_drag, right_interact.interact_pointer_pos())
                    {
                        if drag.clip_id == clip_id && drag.mode == ClipDragMode::ResizeEnd {
                            let pointer_time =
                                view.time_at_screen_x(&layout.timeline_rect, pointer_pos.x);
                            let new_end =
                                drag.anchor_end + (pointer_time - drag.anchor_pointer_time);
                            commands.push(TimelineCommand::ResizeClipEndTo {
                                clip_id,
                                end: new_end,
                            });
                        }
                    }
                    commands.push(TimelineCommand::SelectClip(clip_id));
                    dragged_clip_id = Some(clip_id);
                }
                // 只有当 middle_rect 有实际宽度时才创建 move interact，
                // 否则 resize 边缘占据整个 clip，move 无空间。
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
                        clip_clicked = true;
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
                        };
                        active_clip_drag = Some(drag_state);
                        commands.push(TimelineCommand::SetClipDrag(Some(drag_state)));
                    }
                    if mid_interact.dragged() {
                        if let (Some(drag), Some(pointer_pos)) =
                            (active_clip_drag, mid_interact.interact_pointer_pos())
                        {
                            if drag.clip_id == clip_id && drag.mode == ClipDragMode::Move {
                                let pointer_time =
                                    view.time_at_screen_x(&layout.timeline_rect, pointer_pos.x);
                                let new_start =
                                    drag.anchor_start + (pointer_time - drag.anchor_pointer_time);
                                commands.push(TimelineCommand::MoveClipToStart {
                                    clip_id,
                                    start: new_start,
                                });
                            }
                        }
                        commands.push(TimelineCommand::SelectClip(clip_id));
                        dragged_clip_id = Some(clip_id);
                        if let Some(ptr) = ui.input(|i| i.pointer.hover_pos()) {
                            if ptr.y < track_rect.min.y && track_index > 0 {
                                commands.push(TimelineCommand::MoveClipToTrack {
                                    clip_id,
                                    target_track_index: track_index - 1,
                                });
                            } else if ptr.y > track_rect.max.y {
                                commands.push(TimelineCommand::MoveClipToTrack {
                                    clip_id,
                                    target_track_index: track_index + 1,
                                });
                            }
                        }
                    }
                }
            } else {
                let clip_interact = ui
                    .interact(
                        clip_rect,
                        egui::Id::new(("timeline_clip", clip_id)),
                        egui::Sense::drag(),
                    )
                    .on_hover_cursor(egui::CursorIcon::Grab);
                if clip_interact.clicked() {
                    commands.push(TimelineCommand::SelectClip(clip_id));
                    clip_clicked = true;
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
                    };
                    active_clip_drag = Some(drag_state);
                    commands.push(TimelineCommand::SetClipDrag(Some(drag_state)));
                }
                if clip_interact.dragged() {
                    if let (Some(drag), Some(pointer_pos)) =
                        (active_clip_drag, clip_interact.interact_pointer_pos())
                    {
                        if drag.clip_id == clip_id && drag.mode == ClipDragMode::Move {
                            let pointer_time =
                                view.time_at_screen_x(&layout.timeline_rect, pointer_pos.x);
                            let new_start =
                                drag.anchor_start + (pointer_time - drag.anchor_pointer_time);
                            commands.push(TimelineCommand::MoveClipToStart {
                                clip_id,
                                start: new_start,
                            });
                        }
                    }
                    commands.push(TimelineCommand::SelectClip(clip_id));
                    dragged_clip_id = Some(clip_id);
                    if let Some(ptr) = ui.input(|i| i.pointer.hover_pos()) {
                        if ptr.y < track_rect.min.y && track_index > 0 {
                            commands.push(TimelineCommand::MoveClipToTrack {
                                clip_id,
                                target_track_index: track_index - 1,
                            });
                        } else if ptr.y > track_rect.max.y {
                            commands.push(TimelineCommand::MoveClipToTrack {
                                clip_id,
                                target_track_index: track_index + 1,
                            });
                        }
                    }
                }
            }

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
                    &clip_name,
                    font(10.0),
                    egui::Color32::WHITE,
                );
            }
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

    // 根据当前 clip 拖拽状态设置光标
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

    y + view.track_height
}
