use eframe::egui;
use crate::transport::controller::{apply_track_commands, TrackEditCommand};
use crate::transport::timecode::font;
use crate::transport::{ThemeColors, TimelineState, TimelineView, Track, TrackKind};

pub fn draw_tracks(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    timeline_rect: &egui::Rect,
    state: &mut TimelineState,
    visible_start: f32,
    visible_end: f32,
    ruler_height: f32,
    _scrollbar_height: f32,
    _controls_height: f32,
) -> f32 {
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

    let label_height = 20.0f32;
    let mut y = timeline_rect.min.y + ruler_height;
    let view = &state.view;
    let selected_id = state.selected_clip_id;
    let mut commands = Vec::new();

    if has_video {
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(timeline_rect.min.x, y),
                egui::vec2(timeline_rect.width(), label_height),
            ),
            0.0,
            c.video_label_bg,
        );
        painter.text(
            egui::pos2(timeline_rect.min.x + 8.0, y + label_height / 2.0),
            egui::Align2::LEFT_CENTER,
            "视频",
            font(11.0),
            c.dim_text,
        );
        y += label_height;

        for (track_index, track) in state
            .data
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| track.kind == TrackKind::Video)
        {
            y = draw_track_row(
                ui,
                painter,
                c,
                timeline_rect,
                view,
                selected_id,
                track,
                visible_start,
                visible_end,
                y,
                &mut commands,
                track_index,
            );
        }
    }

    if has_audio {
        y += 4.0;
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(timeline_rect.min.x, y),
                egui::vec2(timeline_rect.width(), label_height),
            ),
            0.0,
            c.audio_label_bg,
        );
        painter.text(
            egui::pos2(timeline_rect.min.x + 8.0, y + label_height / 2.0),
            egui::Align2::LEFT_CENTER,
            "音频",
            font(11.0),
            c.dim_text,
        );
        y += label_height;

        for (track_index, track) in state
            .data
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| track.kind == TrackKind::Audio)
        {
            y = draw_track_row(
                ui,
                painter,
                c,
                timeline_rect,
                view,
                selected_id,
                track,
                visible_start,
                visible_end,
                y,
                &mut commands,
                track_index,
            );
        }
    }

    apply_track_commands(state, commands);

    y
}

fn draw_track_row(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    timeline_rect: &egui::Rect,
    view: &TimelineView,
    selected_id: Option<usize>,
    track: &Track,
    visible_start: f32,
    visible_end: f32,
    y: f32,
    commands: &mut Vec<TrackEditCommand>,
    track_index: usize,
) -> f32 {
    let track_bg = match track.kind {
        TrackKind::Video => c.video_track_bg,
        TrackKind::Audio => c.audio_track_bg,
    };

    let track_rect = egui::Rect::from_min_size(
        egui::pos2(timeline_rect.min.x, y),
        egui::vec2(timeline_rect.width(), view.track_height),
    );
    painter.rect_filled(track_rect, 0.0, track_bg);
    painter.rect_stroke(track_rect, 0.0, egui::Stroke::new(1.0, c.border), egui::StrokeKind::Inside);

    let header_rect = egui::Rect::from_min_size(
        track_rect.min,
        egui::vec2(view.header_width, view.track_height),
    );
    let header_color = if track.muted { c.header_bg_muted } else { c.header_bg };
    painter.rect_filled(header_rect, 0.0, header_color);
    painter.rect_stroke(header_rect, 0.0, egui::Stroke::new(1.0, c.border), egui::StrokeKind::Inside);

    if track.kind == TrackKind::Video {
        let btn_size = 16.0;
        let mute_rect = egui::Rect::from_center_size(
            egui::pos2(header_rect.min.x + 16.0, header_rect.center().y),
            egui::vec2(btn_size, btn_size),
        );
        let mute_color = if track.muted { c.btn_mute_on } else { c.btn_mute_off };
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
        let solo_color = if track.solo { c.btn_solo_on } else { c.btn_solo_off };
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

    let edge_width = 8.0f32;
    let mut clip_clicked = false;
    let mut dragged_clip_id = None;

    for clip_idx in 0..track.clips.len() {
        let clip_start = track.clips[clip_idx].start;
        let clip_end = track.clips[clip_idx].end;
        if clip_end < visible_start || clip_start > visible_end {
            continue;
        }
        let clip_id = track.clips[clip_idx].id;
        let clip_name = track.clips[clip_idx].name.clone();
        let clip_color = track.clips[clip_idx].color;
        let x1 = timeline_rect.min.x + view.header_width + (clip_start - view.scroll_offset) * view.zoom;
        let x2 = timeline_rect.min.x + view.header_width + (clip_end - view.scroll_offset) * view.zoom;
        let clip_rect = egui::Rect::from_min_max(
            egui::pos2(x1.max(track_rect.min.x + view.header_width), track_rect.min.y + 3.0),
            egui::pos2(x2.min(track_rect.max.x), track_rect.max.y - 3.0),
        );
        if clip_rect.width() > 0.0 {
            let is_selected = selected_id == Some(clip_id);

            if is_selected && clip_rect.width() > edge_width * 3.0 {
                let left_edge =
                    egui::Rect::from_min_size(clip_rect.min, egui::vec2(edge_width, clip_rect.height()));
                let right_edge = egui::Rect::from_min_size(
                    egui::pos2(clip_rect.max.x - edge_width, clip_rect.min.y),
                    egui::vec2(edge_width, clip_rect.height()),
                );
                let mid_rect = egui::Rect::from_min_max(
                    egui::pos2(clip_rect.min.x + edge_width, clip_rect.min.y),
                    egui::pos2(clip_rect.max.x - edge_width, clip_rect.max.y),
                );

                let left_interact = ui.interact(left_edge, egui::Id::new(("clip_left", clip_id)), egui::Sense::drag())
                    .on_hover_cursor(egui::CursorIcon::ResizeWest);
                if left_interact.dragged() {
                    let delta = left_interact.drag_delta().x / view.zoom;
                    commands.push(TrackEditCommand::ResizeClipStart { clip_id, delta });
                    commands.push(TrackEditCommand::SelectClip(clip_id));
                    dragged_clip_id = Some(clip_id);
                }
                let right_interact = ui.interact(right_edge, egui::Id::new(("clip_right", clip_id)), egui::Sense::drag())
                    .on_hover_cursor(egui::CursorIcon::ResizeEast);
                if right_interact.dragged() {
                    let delta = right_interact.drag_delta().x / view.zoom;
                    commands.push(TrackEditCommand::ResizeClipEnd { clip_id, delta });
                    commands.push(TrackEditCommand::SelectClip(clip_id));
                    dragged_clip_id = Some(clip_id);
                }
                let mid_interact = ui.interact(mid_rect, egui::Id::new(("clip_mid", clip_id)), egui::Sense::drag())
                    .on_hover_cursor(egui::CursorIcon::Grab);
                if mid_interact.clicked() {
                    commands.push(TrackEditCommand::SelectClip(clip_id));
                    clip_clicked = true;
                }
                if mid_interact.dragged() {
                    let delta = mid_interact.drag_delta().x / view.zoom;
                    commands.push(TrackEditCommand::MoveClip { clip_id, delta });
                    commands.push(TrackEditCommand::SelectClip(clip_id));
                    dragged_clip_id = Some(clip_id);
                    if let Some(ptr) = ui.input(|i| i.pointer.hover_pos()) {
                        if ptr.y < track_rect.min.y && track_index > 0 {
                            commands.push(TrackEditCommand::MoveClipToTrack {
                                clip_id,
                                target_track_index: track_index - 1,
                            });
                        } else if ptr.y > track_rect.max.y {
                            commands.push(TrackEditCommand::MoveClipToTrack {
                                clip_id,
                                target_track_index: track_index + 1,
                            });
                        }
                    }
                }
            } else {
                let clip_interact = ui.interact(clip_rect, egui::Id::new(("timeline_clip", clip_id)), egui::Sense::drag())
                    .on_hover_cursor(egui::CursorIcon::Grab);
                if clip_interact.clicked() {
                    commands.push(TrackEditCommand::SelectClip(clip_id));
                    clip_clicked = true;
                }
                if clip_interact.dragged() {
                    let delta = clip_interact.drag_delta().x / view.zoom;
                    commands.push(TrackEditCommand::MoveClip { clip_id, delta });
                    commands.push(TrackEditCommand::SelectClip(clip_id));
                    dragged_clip_id = Some(clip_id);
                    if let Some(ptr) = ui.input(|i| i.pointer.hover_pos()) {
                        if ptr.y < track_rect.min.y && track_index > 0 {
                            commands.push(TrackEditCommand::MoveClipToTrack {
                                clip_id,
                                target_track_index: track_index - 1,
                            });
                        } else if ptr.y > track_rect.max.y {
                            commands.push(TrackEditCommand::MoveClipToTrack {
                                clip_id,
                                target_track_index: track_index + 1,
                            });
                        }
                    }
                }
            }

            painter.rect_filled(clip_rect, 3.0, clip_color);

            if is_selected {
                painter.rect_stroke(clip_rect, 3.0, egui::Stroke::new(2.0, egui::Color32::WHITE), egui::StrokeKind::Inside);
                let left_edge = egui::Rect::from_min_size(clip_rect.min, egui::vec2(edge_width, clip_rect.height()));
                let right_edge = egui::Rect::from_min_size(
                    egui::pos2(clip_rect.max.x - edge_width, clip_rect.min.y),
                    egui::vec2(edge_width, clip_rect.height()),
                );
                painter.rect_filled(left_edge, 0.0, egui::Color32::from_white_alpha(60));
                painter.rect_filled(right_edge, 0.0, egui::Color32::from_white_alpha(60));
            }

            if clip_rect.width() > 40.0 {
                painter.text(
                    egui::pos2(clip_rect.min.x + edge_width + 2.0, clip_rect.center().y),
                    egui::Align2::LEFT_CENTER, &clip_name, font(10.0), egui::Color32::WHITE,
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
            commands.push(TrackEditCommand::ClearSelection);
        }
    }

    y + view.track_height
}
