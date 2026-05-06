use eframe::egui;

#[derive(Clone, Debug, PartialEq)]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Clone, Debug)]
pub struct TrackClip {
    pub name: String,
    pub start: f32,
    pub end: f32,
    pub color: egui::Color32,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub name: String,
    pub kind: TrackKind,
    pub clips: Vec<TrackClip>,
    pub muted: bool,
    pub solo: bool,
    pub visible: bool,
}

impl Track {
    pub fn new_video(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: TrackKind::Video,
            clips: Vec::new(),
            muted: false,
            solo: false,
            visible: true,
        }
    }

    pub fn new_audio(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: TrackKind::Audio,
            clips: Vec::new(),
            muted: false,
            solo: false,
            visible: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TimelineState {
    pub zoom: f32,
    pub scroll_offset: f32,
    pub track_height: f32,
    pub header_width: f32,
    pub tracks: Vec<Track>,
    pub dragging_playhead: bool,
}

impl Default for TimelineState {
    fn default() -> Self {
        let mut tracks = Vec::new();
        let mut video_track = Track::new_video("视频 1");
        video_track.clips.push(TrackClip {
            name: "主渲染".to_string(),
            start: 0.0,
            end: 0.0,
            color: egui::Color32::from_rgb(80, 120, 200),
        });
        tracks.push(video_track);

        Self {
            zoom: 50.0,
            scroll_offset: 0.0,
            track_height: 36.0,
            header_width: 100.0,
            tracks,
            dragging_playhead: false,
        }
    }
}

impl TimelineState {
    pub fn update_duration(&mut self, duration: f32) {
        for track in &mut self.tracks {
            for clip in &mut track.clips {
                if clip.end > duration || clip.end == 0.0 {
                    clip.end = duration;
                }
            }
        }
    }

    pub fn add_video_track(&mut self, name: &str) {
        self.tracks.push(Track::new_video(name));
    }

    pub fn add_audio_track(&mut self, name: &str) {
        self.tracks.push(Track::new_audio(name));
    }
}

fn font(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Proportional)
}

pub fn show(
    ui: &mut egui::Ui,
    is_playing: &mut bool,
    current_time: &mut f32,
    duration: f32,
    state: &mut TimelineState,
) {
    let available = ui.available_size();
    let response = ui.allocate_response(available, egui::Sense::click_and_drag());
    let rect = response.rect;

    let painter = ui.painter_at(rect);
    let bg_color = egui::Color32::from_rgb(28, 28, 28);
    let video_track_bg = egui::Color32::from_rgb(40, 44, 52);
    let audio_track_bg = egui::Color32::from_rgb(44, 40, 36);
    let ruler_bg = egui::Color32::from_rgb(52, 52, 52);
    let playhead_color = egui::Color32::from_rgb(255, 90, 90);
    let text_color = egui::Color32::from_rgb(220, 220, 220);
    let dim_text = egui::Color32::from_rgb(140, 140, 140);
    let border_color = egui::Color32::from_rgb(60, 60, 60);

    let ruler_height = 26.0;
    let controls_height = 32.0;
    let timeline_rect = rect;

    // 滚轮缩放
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let old_zoom = state.zoom;
            state.zoom = (state.zoom * (1.0 + scroll * 0.001)).clamp(0.2, 500.0);
            if let Some(mouse_pos) = response.hover_pos() {
                let mouse_time = (mouse_pos.x - timeline_rect.min.x - state.header_width) / old_zoom
                    + state.scroll_offset;
                state.scroll_offset = mouse_time
                    - (mouse_pos.x - timeline_rect.min.x - state.header_width) / state.zoom;
            }
        }
    }

    // 拖拽平移
    if response.dragged_by(egui::PointerButton::Middle)
        || (response.dragged_by(egui::PointerButton::Primary)
            && ui.input(|i| i.modifiers.shift))
    {
        state.scroll_offset -= response.drag_delta().x / state.zoom;
    }
    state.scroll_offset = state.scroll_offset.max(0.0);

    // ── 时间标尺 ──
    let ruler_rect = egui::Rect::from_min_size(
        timeline_rect.min,
        egui::vec2(timeline_rect.width(), ruler_height),
    );
    painter.rect_filled(ruler_rect, 0.0, ruler_bg);
    painter.rect_stroke(ruler_rect, 0.0, egui::Stroke::new(1.0, border_color), egui::StrokeKind::Inside);

    let visible_start = state.scroll_offset;
    let visible_end = visible_start + (timeline_rect.width() - state.header_width) / state.zoom;

    let major_interval = if state.zoom > 100.0 {
        1.0
    } else if state.zoom > 50.0 {
        2.0
    } else if state.zoom > 20.0 {
        5.0
    } else if state.zoom > 10.0 {
        10.0
    } else if state.zoom > 5.0 {
        30.0
    } else if state.zoom > 2.0 {
        60.0
    } else if state.zoom > 0.5 {
        120.0
    } else {
        300.0
    };

    let mut t = (visible_start / major_interval).floor() * major_interval;
    while t <= visible_end {
        let x = timeline_rect.min.x + state.header_width + (t - state.scroll_offset) * state.zoom;
        if x >= timeline_rect.min.x + state.header_width {
            painter.line_segment(
                [
                    egui::pos2(x, ruler_rect.min.y + 14.0),
                    egui::pos2(x, ruler_rect.max.y),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 120, 120)),
            );
            let min = (t as u32) / 60;
            let sec = (t as u32) % 60;
            painter.text(
                egui::pos2(x + 3.0, ruler_rect.min.y + 2.0),
                egui::Align2::LEFT_TOP,
                format!("{}:{:02}", min, sec),
                font(11.0),
                text_color,
            );
        }
        t += major_interval;
    }

    // ── 轨道分隔标签 ──
    let mut has_video = false;
    let mut has_audio = false;
    for track in &state.tracks {
        match track.kind {
            TrackKind::Video => has_video = true,
            TrackKind::Audio => has_audio = true,
        }
    }

    let label_height = 20.0f32;
    let mut y = timeline_rect.min.y + ruler_height;

    // ── 视频轨道区域 ──
    if has_video {
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(timeline_rect.min.x, y),
                egui::vec2(timeline_rect.width(), label_height),
            ),
            0.0,
            egui::Color32::from_rgb(50, 55, 65),
        );
        painter.text(
            egui::pos2(timeline_rect.min.x + 8.0, y + label_height / 2.0),
            egui::Align2::LEFT_CENTER,
            "视频",
            font(11.0),
            dim_text,
        );
        y += label_height;

        for track in state.tracks.iter().filter(|t| t.kind == TrackKind::Video) {
            let track_rect = egui::Rect::from_min_size(
                egui::pos2(timeline_rect.min.x, y),
                egui::vec2(timeline_rect.width(), state.track_height),
            );
            painter.rect_filled(track_rect, 0.0, video_track_bg);
            painter.rect_stroke(track_rect, 0.0, egui::Stroke::new(1.0, border_color), egui::StrokeKind::Inside);

            // 轨道头
            let header_rect = egui::Rect::from_min_size(
                track_rect.min,
                egui::vec2(state.header_width, state.track_height),
            );
            let header_color = if track.muted {
                egui::Color32::from_rgb(80, 60, 60)
            } else {
                egui::Color32::from_rgb(55, 60, 70)
            };
            painter.rect_filled(header_rect, 0.0, header_color);
            painter.rect_stroke(header_rect, 0.0, egui::Stroke::new(1.0, border_color), egui::StrokeKind::Inside);

            // 静音/独奏按钮
            let btn_size = 16.0;
            let mute_rect = egui::Rect::from_center_size(
                egui::pos2(header_rect.min.x + 16.0, header_rect.center().y),
                egui::vec2(btn_size, btn_size),
            );
            let mute_color = if track.muted {
                egui::Color32::from_rgb(255, 100, 100)
            } else {
                egui::Color32::from_rgb(100, 100, 100)
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
                egui::Color32::from_rgb(255, 200, 50)
            } else {
                egui::Color32::from_rgb(100, 100, 100)
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
                if track.muted { dim_text } else { text_color },
            );

            // Clips
            for clip in &track.clips {
                if clip.end < visible_start || clip.start > visible_end {
                    continue;
                }
                let x1 = timeline_rect.min.x
                    + state.header_width
                    + (clip.start - state.scroll_offset) * state.zoom;
                let x2 = timeline_rect.min.x
                    + state.header_width
                    + (clip.end - state.scroll_offset) * state.zoom;
                let clip_rect = egui::Rect::from_min_max(
                    egui::pos2(x1.max(track_rect.min.x + state.header_width), track_rect.min.y + 3.0),
                    egui::pos2(x2.min(track_rect.max.x), track_rect.max.y - 3.0),
                );
                if clip_rect.width() > 1.0 {
                    painter.rect_filled(clip_rect, 3.0, clip.color);
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

            y += state.track_height;
        }
    }

    // ── 音频轨道区域 ──
    if has_audio {
        y += 4.0; // 间距
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(timeline_rect.min.x, y),
                egui::vec2(timeline_rect.width(), label_height),
            ),
            0.0,
            egui::Color32::from_rgb(55, 50, 45),
        );
        painter.text(
            egui::pos2(timeline_rect.min.x + 8.0, y + label_height / 2.0),
            egui::Align2::LEFT_CENTER,
            "音频",
            font(11.0),
            dim_text,
        );
        y += label_height;

        for track in state.tracks.iter().filter(|t| t.kind == TrackKind::Audio) {
            let track_rect = egui::Rect::from_min_size(
                egui::pos2(timeline_rect.min.x, y),
                egui::vec2(timeline_rect.width(), state.track_height),
            );
            painter.rect_filled(track_rect, 0.0, audio_track_bg);
            painter.rect_stroke(track_rect, 0.0, egui::Stroke::new(1.0, border_color), egui::StrokeKind::Inside);

            let header_rect = egui::Rect::from_min_size(
                track_rect.min,
                egui::vec2(state.header_width, state.track_height),
            );
            let header_color = if track.muted {
                egui::Color32::from_rgb(80, 60, 60)
            } else {
                egui::Color32::from_rgb(65, 60, 55)
            };
            painter.rect_filled(header_rect, 0.0, header_color);
            painter.rect_stroke(header_rect, 0.0, egui::Stroke::new(1.0, border_color), egui::StrokeKind::Inside);

            painter.text(
                egui::pos2(header_rect.min.x + 8.0, header_rect.center().y),
                egui::Align2::LEFT_CENTER,
                &track.name,
                font(11.0),
                if track.muted { dim_text } else { text_color },
            );

            for clip in &track.clips {
                if clip.end < visible_start || clip.start > visible_end {
                    continue;
                }
                let x1 = timeline_rect.min.x
                    + state.header_width
                    + (clip.start - state.scroll_offset) * state.zoom;
                let x2 = timeline_rect.min.x
                    + state.header_width
                    + (clip.end - state.scroll_offset) * state.zoom;
                let clip_rect = egui::Rect::from_min_max(
                    egui::pos2(x1.max(track_rect.min.x + state.header_width), track_rect.min.y + 3.0),
                    egui::pos2(x2.min(track_rect.max.x), track_rect.max.y - 3.0),
                );
                if clip_rect.width() > 1.0 {
                    painter.rect_filled(clip_rect, 3.0, clip.color);
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

            y += state.track_height;
        }
    }

    // 底部填充
    if y < timeline_rect.max.y - controls_height {
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(timeline_rect.min.x, y),
                egui::pos2(timeline_rect.max.x, timeline_rect.max.y - controls_height),
            ),
            0.0,
            bg_color,
        );
    }

    // ── 播放头 ──
    let playhead_x = timeline_rect.min.x
        + state.header_width
        + (*current_time - state.scroll_offset) * state.zoom;

    let playhead_hit_rect = egui::Rect::from_center_size(
        egui::pos2(playhead_x, timeline_rect.center().y),
        egui::vec2(10.0, timeline_rect.height()),
    );
    let hovering_playhead = response.hover_pos().map_or(false, |p| playhead_hit_rect.contains(p));

    if response.drag_started_by(egui::PointerButton::Primary)
        && hovering_playhead
        && !ui.input(|i| i.modifiers.shift)
    {
        state.dragging_playhead = true;
        *is_playing = false;
    }
    if !response.dragged_by(egui::PointerButton::Primary) {
        state.dragging_playhead = false;
    }

    if state.dragging_playhead {
        if let Some(mouse_pos) = response.interact_pointer_pos() {
            let new_time =
                (mouse_pos.x - timeline_rect.min.x - state.header_width) / state.zoom + state.scroll_offset;
            *current_time = new_time.clamp(0.0, duration);
        }
    }

    // 点击空白处跳转
    if response.clicked_by(egui::PointerButton::Primary)
        && !hovering_playhead
        && !ui.input(|i| i.modifiers.shift)
    {
        if let Some(mouse_pos) = response.hover_pos() {
            if mouse_pos.x > timeline_rect.min.x + state.header_width
                && mouse_pos.y > timeline_rect.min.y + ruler_height
            {
                let new_time =
                    (mouse_pos.x - timeline_rect.min.x - state.header_width) / state.zoom + state.scroll_offset;
                *current_time = new_time.clamp(0.0, duration);
                *is_playing = false;
            }
        }
    }

    if playhead_x >= timeline_rect.min.x + state.header_width {
        painter.line_segment(
            [
                egui::pos2(playhead_x, timeline_rect.min.y),
                egui::pos2(playhead_x, timeline_rect.max.y - controls_height),
            ],
            egui::Stroke::new(2.0, playhead_color),
        );
        let tri = vec![
            egui::pos2(playhead_x - 7.0, timeline_rect.min.y),
            egui::pos2(playhead_x + 7.0, timeline_rect.min.y),
            egui::pos2(playhead_x, timeline_rect.min.y + 9.0),
        ];
        painter.add(egui::Shape::convex_polygon(tri, playhead_color, egui::Stroke::NONE));
    }

    // ── 底部控制栏 ──
    let controls_rect = egui::Rect::from_min_max(
        egui::pos2(timeline_rect.min.x, timeline_rect.max.y - controls_height),
        timeline_rect.max,
    );
    painter.rect_filled(controls_rect, 0.0, egui::Color32::from_rgb(35, 35, 35));
    painter.rect_stroke(controls_rect, 0.0, egui::Stroke::new(1.0, border_color), egui::StrokeKind::Inside);

    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(controls_rect));
    child_ui.horizontal(|ui| {
        ui.add_space(8.0);
        if ui.button(if *is_playing { "⏸" } else { "▶" }).clicked() {
            *is_playing = !*is_playing;
        }
        if ui.button("⏹").clicked() {
            *is_playing = false;
            *current_time = 0.0;
        }
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new(format!("{:06.2} / {:06.2}", *current_time, duration))
                .font(font(12.0)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("缩放: {:.0}px/s", state.zoom))
                    .font(font(11.0))
                    .color(egui::Color32::GRAY),
            );
        });
    });
}
