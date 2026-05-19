use eframe::egui;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Clone, Copy, Debug, PartialEq)]
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
    pub equal_key_width: bool,
    pub midi_idx: Option<usize>,
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
}

#[derive(Clone, Debug)]
pub struct Track {
    pub name: String,
    pub kind: TrackKind,
    pub clips: Vec<TrackClip>,
    pub muted: bool,
    pub solo: bool,
}

impl Track {
    pub fn new_video(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: TrackKind::Video,
            clips: Vec::new(),
            muted: false,
            solo: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollbarDrag {
    Pan {
        anchor_time: f32,
        anchor_vis_start: f32,
    },
    LeftEdge,
    RightEdge,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClipDragMode {
    Move,
    ResizeStart,
    ResizeEnd,
}

#[derive(Clone, Copy, Debug)]
pub struct ClipDragState {
    pub clip_id: usize,
    pub mode: ClipDragMode,
    pub anchor_pointer_time: f32,
    pub anchor_start: f32,
    pub anchor_end: f32,
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

impl TimelineView {
    pub fn visible_range(&self, content_width: f32) -> (f32, f32) {
        let visible_start = self.scroll_offset;
        let visible_end = visible_start + content_width / self.zoom;
        (visible_start, visible_end)
    }

    pub fn time_at_screen_x(&self, timeline_rect: &egui::Rect, x: f32) -> f32 {
        (x - timeline_rect.min.x - self.header_width) / self.zoom + self.scroll_offset
    }

    pub fn screen_x_for_time(&self, timeline_rect: &egui::Rect, time: f32) -> f32 {
        timeline_rect.min.x + self.header_width + (time - self.scroll_offset) * self.zoom
    }

    pub fn zoom_around_pointer(
        &mut self,
        timeline_rect: &egui::Rect,
        pointer_x: f32,
        zoom_factor: f32,
    ) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * zoom_factor).clamp(0.2, 5000.0);
        let mouse_time =
            (pointer_x - timeline_rect.min.x - self.header_width) / old_zoom + self.scroll_offset;
        self.scroll_offset =
            mouse_time - (pointer_x - timeline_rect.min.x - self.header_width) / self.zoom;
        self.clamp_scroll();
    }

    pub fn pan_by_pixels(&mut self, pixels: f32) {
        self.scroll_offset -= pixels / self.zoom;
        self.clamp_scroll();
    }

    pub fn clamp_scroll(&mut self) {
        self.scroll_offset = self.scroll_offset.max(0.0);
    }
}

#[derive(Clone, Debug)]
pub struct TimelineData {
    pub tracks: Vec<Track>,
    pub next_track_id: usize,
}

impl Default for TimelineData {
    fn default() -> Self {
        let mut tracks = Vec::new();
        let mut video_track = Track::new_video("视频 1");
        video_track.clips.push(TrackClip::new_waterfall(1, None));
        tracks.push(video_track);
        Self {
            tracks,
            next_track_id: 2,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TimelineInteraction {
    pub dragging_playhead: bool,
    pub scrollbar_drag: Option<ScrollbarDrag>,
    pub clip_drag: Option<ClipDragState>,
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
            next_clip_id: 2,
        }
    }
}

impl TimelineState {
    /// 计算时间线有内容的最后一帧（所有 clip `end` 的最大值）。
    pub fn content_duration(&self) -> f32 {
        self.data
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .map(|c| c.end)
            .fold(0.0, f32::max)
    }

    /// 仅对尚未设置长度的 clip（end == 0）设置默认长度，
    /// 不会截断已经存在的 clip。
    pub fn update_duration(&mut self, duration: f32) {
        for track in &mut self.data.tracks {
            for clip in &mut track.clips {
                if clip.end == 0.0 {
                    clip.end = duration;
                }
            }
        }
    }

    pub fn push_waterfall_clip(&mut self, midi_idx: Option<usize>, duration: f32) {
        let id = self.next_clip_id;
        self.next_clip_id += 1;
        let track_id = self.data.next_track_id;
        self.data.next_track_id += 1;
        let mut track = Track::new_video(&format!("视频 {}", track_id));
        let mut clip = TrackClip::new_waterfall(id, midi_idx);
        clip.end = if duration > 0.0 { duration } else { 5.0 };
        track.clips.push(clip);
        self.data.tracks.insert(0, track);
    }

    pub fn push_solid_color_clip(&mut self, color: egui::Color32, duration: f32) {
        let id = self.next_clip_id;
        self.next_clip_id += 1;
        let track_id = self.data.next_track_id;
        self.data.next_track_id += 1;
        let mut track = Track::new_video(&format!("视频 {}", track_id));
        let mut clip = TrackClip::new_solid_color(id, color);
        clip.end = if duration > 0.0 { duration } else { 5.0 };
        track.clips.push(clip);
        self.data.tracks.insert(0, track);
    }

    pub fn remove_selected_clip(&mut self) {
        let Some(id) = self.selected_clip_id else {
            return;
        };
        for track in &mut self.data.tracks {
            track.clips.retain(|clip| clip.id != id);
        }
        self.selected_clip_id = None;
    }

    pub fn select_clip(&mut self, clip_id: usize) {
        self.selected_clip_id = Some(clip_id);
    }

    pub fn clear_selection(&mut self) {
        self.selected_clip_id = None;
    }

    pub fn move_clip_to_start(&mut self, clip_id: usize, new_start: f32) {
        let frame_duration = self.frame_duration();
        if let Some(clip) = self.find_clip_mut(clip_id) {
            let width = clip.end - clip.start;
            clip.start = snap_to_frame(new_start.max(0.0), frame_duration);
            clip.end = clip.start + width;
        }
    }

    pub fn resize_clip_start_to(&mut self, clip_id: usize, new_start: f32) {
        let frame_duration = self.frame_duration();
        if let Some(clip) = self.find_clip_mut(clip_id) {
            clip.start = snap_to_frame(new_start.max(0.0), frame_duration);
            clip.start = clip.start.min(clip.end - frame_duration);
        }
    }

    pub fn resize_clip_end_to(&mut self, clip_id: usize, new_end: f32) {
        let frame_duration = self.frame_duration();
        if let Some(clip) = self.find_clip_mut(clip_id) {
            clip.end = snap_to_frame(new_end.max(clip.start + frame_duration), frame_duration);
        }
    }

    pub fn move_clip_to_track(&mut self, clip_id: usize, target_track_index: usize) {
        let mut clip_to_move = None;
        for track in &mut self.data.tracks {
            if let Some(pos) = track.clips.iter().position(|clip| clip.id == clip_id) {
                clip_to_move = Some(track.clips.remove(pos));
                break;
            }
        }

        let Some(clip) = clip_to_move else {
            return;
        };

        let dest_index = if target_track_index < self.data.tracks.len() {
            if self.data.tracks[target_track_index].kind == TrackKind::Video {
                Some(target_track_index)
            } else {
                self.data
                    .tracks
                    .iter()
                    .position(|t| t.kind == TrackKind::Video)
            }
        } else {
            let name = format!("视频 {}", target_track_index + 1);
            self.data.tracks.push(Track::new_video(&name));
            Some(self.data.tracks.len() - 1)
        };

        if let Some(idx) = dest_index {
            self.data.tracks[idx].clips.push(clip);
        } else {
            let mut track = Track::new_video("视频 1");
            track.clips.push(clip);
            self.data.tracks.push(track);
        }

        self.data.tracks.retain(|track| !track.clips.is_empty());
    }

    fn frame_duration(&self) -> f32 {
        1.0 / self.fps.max(1) as f32
    }

    fn find_clip_mut(&mut self, clip_id: usize) -> Option<&mut TrackClip> {
        for track in &mut self.data.tracks {
            if let Some(clip) = track.clips.iter_mut().find(|clip| clip.id == clip_id) {
                return Some(clip);
            }
        }
        None
    }
}

fn snap_to_frame(time: f32, frame_duration: f32) -> f32 {
    if frame_duration <= 0.0 {
        return time.max(0.0);
    }
    (time / frame_duration).round() * frame_duration
}
