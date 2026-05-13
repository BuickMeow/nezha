use eframe::egui;

#[derive(Clone, Debug, PartialEq)]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClipKind {
    Waterfall,
    SolidColor,
}

#[derive(Clone, Debug)]
pub struct TrackClip {
    pub id: usize,
    pub name: String,
    pub kind: ClipKind,
    pub start: f32,
    pub end: f32,
    pub color: egui::Color32,
    pub speed: f32,
    pub border_width: f32,
    pub rounding: f32,
    pub render_mode: nezha_renderer::RenderMode,
    /// 是否等宽钢琴键（false = 白键比黑键宽，黑键在白键上方）
    pub equal_key_width: bool,
    /// 关联的 MIDI 索引（None = 使用当前高亮 MIDI）
    pub midi_idx: Option<usize>,
    /// 钢琴键盘高度占渲染高度的比例 (0.0 ~ 0.5)
    pub keyboard_height_percent: f32,
}

impl TrackClip {
    pub fn new_waterfall(id: usize, midi_idx: Option<usize>) -> Self {
        Self {
            id,
            name: format!("默认瀑布流 {}", id),
            kind: ClipKind::Waterfall,
            start: 0.0,
            end: 0.0,
            color: egui::Color32::from_rgb(80, 150, 220),
            speed: 1.0,
            border_width: 0.1,
            rounding: 0.0,
            render_mode: nezha_renderer::RenderMode::TimeBased,
            equal_key_width: true,
            midi_idx,
            keyboard_height_percent: 0.15,
        }
    }

    pub fn new_solid_color(id: usize, color: egui::Color32) -> Self {
        Self {
            id,
            name: format!("纯色 {}", id),
            kind: ClipKind::SolidColor,
            start: 0.0,
            end: 0.0,
            color,
            speed: 1.0,
            border_width: 0.0,
            rounding: 0.0,
            render_mode: nezha_renderer::RenderMode::TimeBased,
            equal_key_width: true,
            midi_idx: None,
            keyboard_height_percent: 0.0,
        }
    }

    /// 默认渲染参数：(border_width, rounding, track_index, render_mode, equal_key, keyboard%)
    pub fn default_render_params() -> (f32, f32, usize, nezha_renderer::RenderMode, bool, f32) {
        (
            0.1,
            0.0,
            0,
            nezha_renderer::RenderMode::TimeBased,
            true,
            0.15,
        )
    }
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
        video_track.clips.push(TrackClip::new_waterfall(0, None));
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

    /// 查找当前选中的 clip
    pub fn selected_clip(&self) -> Option<&TrackClip> {
        let id = self.selected_clip_id?;
        self.data
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .find(|c| c.id == id)
    }

    /// 查找当前时间点的纯色图层（用于背景色）
    pub fn solid_color_at(&self, time_secs: f32) -> Option<&TrackClip> {
        self.data
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .find(|c| c.kind == ClipKind::SolidColor && time_secs >= c.start && time_secs < c.end)
    }

    /// 添加瀑布流 clip（自动分配 id、新建 track、插入到时间轴顶部）
    pub fn push_waterfall_clip(&mut self, midi_idx: Option<usize>, duration: f32) {
        let id = self.next_clip_id;
        self.next_clip_id += 1;
        let track_len = self.data.tracks.len();
        let mut track = Track::new_video(&format!("视频 {}", track_len + 1));
        let mut clip = TrackClip::new_waterfall(id, midi_idx);
        clip.end = duration.max(1.0);
        track.clips.push(clip);
        self.data.tracks.insert(0, track);
    }

    /// 添加纯色 clip（自动分配 id、新建 track、插入到时间轴顶部）
    pub fn push_solid_color_clip(&mut self, color: egui::Color32, duration: f32) {
        let id = self.next_clip_id;
        self.next_clip_id += 1;
        let track_len = self.data.tracks.len();
        let mut track = Track::new_video(&format!("视频 {}", track_len + 1));
        let mut clip = TrackClip::new_solid_color(id, color);
        clip.end = duration.max(1.0);
        track.clips.push(clip);
        self.data.tracks.insert(0, track);
    }
}

// ── 子模块 ──

mod controls;
mod input;
mod playhead;
mod ruler;
mod scrollbar;
mod theme;
mod timecode;
mod tracks;

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

    let visible_start = state.view.scroll_offset;
    let visible_end = visible_start + content_width / state.view.zoom;

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
