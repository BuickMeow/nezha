use eframe::egui;

#[derive(Clone, Debug, PartialEq)]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Clone, Debug)]
pub struct TrackClip {
    pub id: usize,
    pub name: String,
    pub start: f32,
    pub end: f32,
    pub color: egui::Color32,
    pub speed: f32,
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

#[derive(Clone, Debug, PartialEq)]
pub enum ScrollbarDrag {
    Pan { anchor_time: f32 },
    LeftEdge,
    RightEdge,
}

#[derive(Clone, Debug)]
pub struct TimelineView {
    pub zoom: f32,
    pub scroll_offset: f32,
    pub track_height: f32,
    pub header_width: f32,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self {
            zoom: 50.0,
            scroll_offset: 0.0,
            track_height: 36.0,
            header_width: 100.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TimelineData {
    pub tracks: Vec<Track>,
}

impl Default for TimelineData {
    fn default() -> Self {
        let mut tracks = Vec::new();
        let mut video_track = Track::new_video("视频 1");
        video_track.clips.push(TrackClip {
            id: 0,
            name: "主渲染".to_string(),
            start: 0.0,
            end: 0.0,
            color: egui::Color32::from_rgb(80, 120, 200),
            speed: 1.0,
        });
        tracks.push(video_track);
        Self { tracks }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TimelineInteraction {
    pub dragging_playhead: bool,
    pub scrollbar_drag: Option<ScrollbarDrag>,
}

#[derive(Clone, Debug)]
pub struct TimelineState {
    pub view: TimelineView,
    pub data: TimelineData,
    pub interaction: TimelineInteraction,
    pub fps: u32,
    pub selected_clip_id: Option<usize>,
    pub next_clip_id: usize,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            view: TimelineView::default(),
            data: TimelineData::default(),
            interaction: TimelineInteraction::default(),
            fps: 60,
            selected_clip_id: None,
            next_clip_id: 1,
        }
    }
}

impl TimelineState {
    pub fn update_duration(&mut self, duration: f32) {
        for track in &mut self.data.tracks {
            for clip in &mut track.clips {
                if clip.end > duration || clip.end == 0.0 {
                    clip.end = duration;
                }
            }
        }
    }

    pub fn add_video_track(&mut self, name: &str) {
        self.data.tracks.push(Track::new_video(name));
    }

    pub fn add_audio_track(&mut self, name: &str) {
        self.data.tracks.push(Track::new_audio(name));
    }
}

// ── 子模块 ──

mod theme;
mod timecode;
mod input;
mod ruler;
mod scrollbar;
mod tracks;
mod playhead;
mod controls;

pub use theme::ThemeColors;

use input::handle_input;
use ruler::draw_ruler;
use scrollbar::draw_scrollbar;
use tracks::draw_tracks;
use playhead::draw_playhead;
use controls::draw_controls;

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

    let visible_start = state.view.scroll_offset;
    let visible_end = visible_start + content_width / state.view.zoom;

    // ── 标尺 ──
    draw_ruler(
        ui, &painter, &c, &timeline_rect, state, visible_start, visible_end, ruler_height,
        &response, current_time, duration, fps,
    );

    // ── 滚动条 ──
    draw_scrollbar(
        ui, &painter, &c, &timeline_rect, &mut state.view, &mut state.interaction,
        duration, content_width, scrollbar_height, controls_height, &response, fps,
    );

    // ── 轨道 ──
    let y = draw_tracks(
        ui, &painter, &c, &timeline_rect, state, visible_start, visible_end,
        ruler_height, scrollbar_height, controls_height,
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
        ui, &painter, &c, &timeline_rect, &response, state, current_time, duration,
        ruler_height, controls_height, scrollbar_height, fps,
    );

    // ── 底部控制栏 ──
    draw_controls(
        ui, &painter, &c, &timeline_rect, is_playing, current_time, duration, state,
        controls_height,
    );
}
