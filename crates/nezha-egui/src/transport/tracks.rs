use eframe::egui;
use crate::transport::{TimelineState, TimelineView, Track, TrackKind, ThemeColors};
use crate::transport::timecode::font;

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
    let mut has_video = false;
    let mut has_audio = false;
    for track in &state.data.tracks {
        match track.kind {
            TrackKind::Video => has_video = true,
            TrackKind::Audio => has_audio = true,
        }
    }

    let label_height = 20.0f32;
    let mut y = timeline_rect.min.y + ruler_height;
    let view = &state.view;
    let selected_id = &mut state.selected_clip_id;

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

        for track in state.data.tracks.iter().filter(|t| t.kind == TrackKind::Video) {
            y = draw_track_row(ui, painter, c, timeline_rect, view, selected_id, track, visible_start, visible_end, y);
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

        for track in state.data.tracks.iter().filter(|t| t.kind == TrackKind::Audio) {
            y = draw_track_row(ui, painter, c, timeline_rect, view, selected_id, track, visible_start, visible_end, y);
        }
    }

    y
}

fn draw_track_row(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    timeline_rect: &egui::Rect,
    view: &TimelineView,
    selected_id: &mut Option<usize>,
    track: &Track,
    visible_start: f32,
    visible_end: f32,
    y: f32,
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

    let mut clip_clicked = false;

    for clip in &track.clips {
        if clip.end < visible_start || clip.start > visible_end {
            continue;
        }
        let x1 = timeline_rect.min.x
            + view.header_width
            + (clip.start - view.scroll_offset) * view.zoom;
        let x2 = timeline_rect.min.x
            + view.header_width
            + (clip.end - view.scroll_offset) * view.zoom;
        let clip_rect = egui::Rect::from_min_max(
            egui::pos2(x1.max(track_rect.min.x + view.header_width), track_rect.min.y + 3.0),
            egui::pos2(x2.min(track_rect.max.x), track_rect.max.y - 3.0),
        );
        if clip_rect.width() > 1.0 {
            let is_selected = *selected_id == Some(clip.id);

            let clip_interact = ui.interact(
                clip_rect,
                egui::Id::new(("timeline_clip", clip.id)),
                egui::Sense::click(),
            );
            if clip_interact.clicked() {
                *selected_id = Some(clip.id);
                clip_clicked = true;
            }

            painter.rect_filled(clip_rect, 3.0, clip.color);

            if is_selected {
                painter.rect_stroke(
                    clip_rect,
                    3.0,
                    egui::Stroke::new(2.0, egui::Color32::WHITE),
                    egui::StrokeKind::Inside,
                );
            }

            if clip_rect.width() > 40.0 {
                painter.text(
                    egui::pos2(clip_rect.min.x + 4.0, clip_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &clip.name,
                    font(10.0),
                    egui::Color32::WHITE,
                );
            }
        }
    }

    // 点击本 clip 之外的区域取消选择（使用 track 底部留白区域）
    if !clip_clicked {
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
            *selected_id = None;
        }
    }

    y + view.track_height
}
