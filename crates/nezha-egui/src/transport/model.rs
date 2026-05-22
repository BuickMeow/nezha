use eframe::egui;
pub use nezha_compositor::BlendMode;

// ── 枚举与基础类型 ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrackKind {
    Video,
    Audio,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClipKind {
    Waterfall,
    SolidColor,
    /// 音符计数器图层：显示当前时间和可见音符数的浮动文本。
    Counter,
}

/// 所有图层共有的变换与合成属性。
#[derive(Clone, Debug)]
pub struct LayerCommon {
    /// 屏幕 X 位置（像素）。
    pub position_x: f32,
    /// 屏幕 Y 位置（像素）。
    pub position_y: f32,
    /// 水平缩放（1.0 = 原始大小，负数 = 水平翻转）。
    pub scale_x: f32,
    /// 垂直缩放（1.0 = 原始大小，负数 = 垂直翻转）。
    pub scale_y: f32,
    /// 是否锁定横纵比（调整任一项时同步修改另一项）。
    pub scale_linked: bool,
    /// 合成方式。
    pub blend_mode: BlendMode,
    /// 不透明度（0.0 = 完全透明，1.0 = 完全不透明）。
    pub opacity: f32,
}

impl Default for LayerCommon {
    fn default() -> Self {
        Self {
            position_x: 0.0,
            position_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            scale_linked: false,
            blend_mode: BlendMode::Normal,
            opacity: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TrackClip {
    pub id: usize,
    pub name: String,
    pub kind: ClipKind,
    pub start: f32,
    pub end: f32,
    pub color: egui::Color32,
    /// 渲染图层时使用的文字颜色（仅 Counter 等文字图层生效）。
    pub text_color: egui::Color32,
    pub speed: f32,
    pub border_width: f32,
    pub rounding: f32,
    pub render_mode: nezha_renderer::RenderMode,
    pub equal_key_width: bool,
    pub midi_idx: Option<usize>,
    pub keyboard_height_percent: f32,
    /// 计数器/文本图层的字号（像素）。
    pub font_size: u32,
    /// 所有图层共有的变换与合成属性。
    pub common: LayerCommon,
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
            text_color: egui::Color32::WHITE,
            speed: 1.0,
            border_width: 0.1,
            rounding: 0.0,
            render_mode: nezha_renderer::RenderMode::TimeBased,
            equal_key_width: false,
            midi_idx,
            keyboard_height_percent: 0.15,
            font_size: 24,
            common: LayerCommon::default(),
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
            text_color: egui::Color32::WHITE,
            speed: 1.0,
            border_width: 0.0,
            rounding: 0.0,
            render_mode: nezha_renderer::RenderMode::TimeBased,
            equal_key_width: false,
            midi_idx: None,
            keyboard_height_percent: 0.0,
            font_size: 24,
            common: LayerCommon::default(),
        }
    }

    pub fn new_counter(id: usize) -> Self {
        Self {
            id,
            name: format!("计数器 {}", id),
            kind: ClipKind::Counter,
            start: 0.0,
            end: 0.0,
            color: egui::Color32::from_rgb(0xBB, 0xB0, 0x94),
            text_color: egui::Color32::WHITE,
            speed: 1.0,
            border_width: 0.0,
            rounding: 0.0,
            render_mode: nezha_renderer::RenderMode::TimeBased,
            equal_key_width: false,
            midi_idx: None,
            keyboard_height_percent: 0.0,
            font_size: 24,
            common: LayerCommon {
                position_x: 20.0,
                position_y: 20.0,
                ..Default::default()
            },
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

// ── 拖拽状态 ─────────────────────────────────────────────────────────────────

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
    /// 本次拖拽中是否已新建过轨道（避免每帧重复创建）。
    pub track_was_inserted: bool,
}

// ── 交互状态 ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct TimelineInteraction {
    pub dragging_playhead: bool,
    pub scrollbar_drag: Option<ScrollbarDrag>,
    pub clip_drag: Option<ClipDragState>,
}

// ── 选区状态 ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct TimelineSelection {
    pub selected_clip_id: Option<usize>,
}

impl TimelineSelection {
    pub fn select(&mut self, clip_id: usize) {
        self.selected_clip_id = Some(clip_id);
    }

    pub fn clear(&mut self) {
        self.selected_clip_id = None;
    }
}

// ── 视图状态 ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct TimelineView {
    pub zoom: f32,
    pub scroll_offset: f32,
    pub scroll_y: f32,
    pub track_height: f32,
    pub header_width: f32,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self {
            zoom: 50.0,
            scroll_offset: 0.0,
            scroll_y: 0.0,
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

    pub fn clamp_scroll_y(&mut self, track_area_height: f32, total_track_height: f32) {
        // 留出半个轨道高度的底部余量，便于最后一条轨道完全可见
        let bottom_margin = self.track_height * 0.5;
        let max_scroll = (total_track_height + bottom_margin - track_area_height).max(0.0);
        self.scroll_y = self.scroll_y.clamp(0.0, max_scroll);
    }
}

// ── 数据模型 ─────────────────────────────────────────────────────────────────

/// 根据已有轨道数量自动生成下一个视频轨道的名称。
pub fn next_video_track_name(tracks: &[Track]) -> String {
    let count = tracks.iter().filter(|t| t.kind == TrackKind::Video).count();
    format!("视频 {}", count + 1)
}

#[derive(Clone, Debug)]
pub struct TimelineData {
    pub tracks: Vec<Track>,
    next_clip_id: usize,
}

impl Default for TimelineData {
    fn default() -> Self {
        Self {
            tracks: vec![Track::new_video(&next_video_track_name(&[]))],
            next_clip_id: 1,
        }
    }
}

impl TimelineData {
    // ── ID 分配 ──

    /// 分配一个新的 clip ID。
    pub fn alloc_clip_id(&mut self) -> usize {
        let id = self.next_clip_id;
        self.next_clip_id += 1;
        id
    }

    fn recompute_next_clip_id(&mut self) {
        let max_id = self
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .map(|c| c.id)
            .max()
            .unwrap_or(0);
        self.next_clip_id = max_id + 1;
    }

    // ── 推入 clip ──

    /// Push a new clip onto a new track at the top of the timeline.
    /// Returns the assigned clip ID.
    pub fn push_clip(
        &mut self,
        kind: ClipKind,
        duration: f32,
        midi_idx: Option<usize>,
        color: egui::Color32,
    ) -> usize {
        let id = self.alloc_clip_id();

        let type_count = self
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .filter(|c| c.kind == kind)
            .count()
            + 1;
        let mut clip = match kind {
            ClipKind::Waterfall => {
                let mut c = TrackClip::new_waterfall(id, midi_idx);
                c.name = format!("默认瀑布流 {}", type_count);
                c
            }
            ClipKind::SolidColor => {
                let mut c = TrackClip::new_solid_color(id, color);
                c.name = format!("纯色 {}", type_count);
                c
            }
            ClipKind::Counter => {
                let mut c = TrackClip::new_counter(id);
                c.name = format!("计数器 {}", type_count);
                c
            }
        };
        clip.end = if duration > 0.0 { duration } else { 5.0 };

        // 找第一个空的视频轨道（可能被删光 clip 后留下），没有则新建一条
        if let Some(empty_track) = self
            .tracks
            .iter_mut()
            .find(|t| t.kind == TrackKind::Video && t.clips.is_empty())
        {
            empty_track.clips.push(clip);
        } else {
            let mut track = Track::new_video(&next_video_track_name(&self.tracks));
            track.clips.push(clip);
            self.tracks.insert(0, track);
        }
        id
    }

    /// Convenience wrapper to push a waterfall clip.
    pub fn push_waterfall_clip(&mut self, midi_idx: Option<usize>, duration: f32) -> usize {
        self.push_clip(
            ClipKind::Waterfall,
            duration,
            midi_idx,
            egui::Color32::TRANSPARENT,
        )
    }

    /// Convenience wrapper to push a solid color clip.
    pub fn push_solid_color_clip(&mut self, color: egui::Color32, duration: f32) -> usize {
        self.push_clip(ClipKind::SolidColor, duration, None, color)
    }

    /// Convenience wrapper to push a counter clip.
    pub fn push_counter_clip(&mut self, duration: f32) -> usize {
        self.push_clip(
            ClipKind::Counter,
            duration,
            None,
            egui::Color32::TRANSPARENT,
        )
    }

    // ── 删除 clip ──

    /// Remove a clip by ID. Returns whether the clip was found.
    pub fn remove_clip(&mut self, clip_id: usize) -> bool {
        let mut found = false;
        for track in &mut self.tracks {
            let before = track.clips.len();
            track.clips.retain(|clip| clip.id != clip_id);
            if track.clips.len() < before {
                found = true;
            }
        }
        self.recompute_next_clip_id();
        found
    }

    // ── 查询 ──

    /// 计算时间线有内容的最后一帧（所有 clip `end` 的最大值）。
    pub fn content_duration(&self) -> f32 {
        self.tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .map(|c| c.end)
            .fold(0.0, f32::max)
    }

    /// 仅对尚未设置长度的 clip（end == 0）设置默认长度，
    /// 不会截断已经存在的 clip。
    pub fn update_duration(&mut self, duration: f32) {
        for track in &mut self.tracks {
            for clip in &mut track.clips {
                if clip.end == 0.0 {
                    clip.end = duration;
                }
            }
        }
    }

    // ── 修改 clip ──

    pub fn move_clip_to_start(&mut self, clip_id: usize, new_start: f32, fps: u32) {
        let frame_duration = Self::frame_duration(fps);
        if let Some(clip) = self.find_clip_mut(clip_id) {
            let width = clip.end - clip.start;
            clip.start = snap_to_frame(new_start.max(0.0), frame_duration);
            clip.end = clip.start + width;
        }
    }

    pub fn resize_clip_start_to(&mut self, clip_id: usize, new_start: f32, fps: u32) {
        let frame_duration = Self::frame_duration(fps);
        if let Some(clip) = self.find_clip_mut(clip_id) {
            clip.start = snap_to_frame(new_start.max(0.0), frame_duration);
            clip.start = clip.start.min(clip.end - frame_duration);
        }
    }

    pub fn resize_clip_end_to(&mut self, clip_id: usize, new_end: f32, fps: u32) {
        let frame_duration = Self::frame_duration(fps);
        if let Some(clip) = self.find_clip_mut(clip_id) {
            clip.end = snap_to_frame(new_end.max(clip.start + frame_duration), frame_duration);
        }
    }

    pub fn move_clip_to_track(
        &mut self,
        clip_id: usize,
        target_track_index: usize,
        interaction: &mut TimelineInteraction,
    ) {
        let mut src_track_idx = None;
        let mut clip_to_move = None;
        'outer: for (i, track) in self.tracks.iter_mut().enumerate() {
            for (j, clip) in track.clips.iter().enumerate() {
                if clip.id == clip_id {
                    clip_to_move = Some(track.clips.remove(j));
                    src_track_idx = Some(i);
                    break 'outer;
                }
            }
        }

        let Some(clip) = clip_to_move else {
            return;
        };

        // 检查本次拖拽是否已经新建过轨道，避免每帧重复插入
        let track_already_inserted = interaction
            .clip_drag
            .map(|d| d.track_was_inserted)
            .unwrap_or(false);

        let dest_index = if target_track_index == 0 {
            if src_track_idx == Some(0) && !track_already_inserted {
                // 从第一个轨道拖到上方 → 插入新轨道在位置 0
                let video_count = self
                    .tracks
                    .iter()
                    .filter(|t| t.kind == TrackKind::Video)
                    .count();
                let name = format!("视频 {}", video_count + 1);
                self.tracks.insert(0, Track::new_video(&name));
                // 标记已插入，后续帧不再重复建轨道
                if let Some(ref mut drag) = interaction.clip_drag {
                    drag.track_was_inserted = true;
                }
                Some(0)
            } else {
                // 从其他轨道拖到轨道 0 → 移到已有轨道 0
                if self.tracks[0].kind == TrackKind::Video {
                    Some(0)
                } else {
                    self.tracks.iter().position(|t| t.kind == TrackKind::Video)
                }
            }
        } else if target_track_index < self.tracks.len() {
            if self.tracks[target_track_index].kind == TrackKind::Video {
                Some(target_track_index)
            } else {
                self.tracks.iter().position(|t| t.kind == TrackKind::Video)
            }
        } else {
            // 超出范围 → 放回来源轨道（不做跨轨道移动）
            src_track_idx
        };

        if let Some(idx) = dest_index {
            self.tracks[idx].clips.push(clip);
        } else if src_track_idx.is_none() {
            // 既无目标也无来源，兜底建新轨道
            let name = format!(
                "视频 {}",
                self.tracks
                    .iter()
                    .filter(|t| t.kind == TrackKind::Video)
                    .count()
                    + 1
            );
            let mut track = Track::new_video(&name);
            track.clips.push(clip);
            self.tracks.push(track);
        }
    }

    // ── 内部辅助 ──

    fn frame_duration(fps: u32) -> f32 {
        1.0 / fps.max(1) as f32
    }

    fn find_clip_mut(&mut self, clip_id: usize) -> Option<&mut TrackClip> {
        for track in &mut self.tracks {
            if let Some(clip) = track.clips.iter_mut().find(|clip| clip.id == clip_id) {
                return Some(clip);
            }
        }
        None
    }
}

// ── 顶层状态（组合） ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct TimelineState {
    pub view: TimelineView,
    pub data: TimelineData,
    pub interaction: TimelineInteraction,
    pub selection: TimelineSelection,
    pub fps: u32,
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            view: TimelineView::default(),
            data: TimelineData::default(),
            interaction: TimelineInteraction::default(),
            selection: TimelineSelection::default(),
            fps: 60,
        }
    }
}

impl TimelineState {
    pub fn push_waterfall_clip(&mut self, midi_idx: Option<usize>, duration: f32) {
        let id = self.data.push_waterfall_clip(midi_idx, duration);
        self.selection.select(id);
    }

    pub fn push_solid_color_clip(&mut self, color: egui::Color32, duration: f32) {
        let id = self.data.push_solid_color_clip(color, duration);
        self.selection.select(id);
    }

    pub fn push_counter_clip(&mut self, duration: f32) {
        let id = self.data.push_counter_clip(duration);
        self.selection.select(id);
    }

    /// 删除当前选中的 clip。
    pub fn remove_selected_clip(&mut self) {
        if let Some(id) = self.selection.selected_clip_id {
            self.data.remove_clip(id);
            self.selection.clear();
        }
    }

    /// 移动 clip 开始时间（使用 state 的 fps 做吸附）。
    pub fn move_clip_to_start(&mut self, clip_id: usize, new_start: f32) {
        self.data.move_clip_to_start(clip_id, new_start, self.fps);
    }

    pub fn resize_clip_start_to(&mut self, clip_id: usize, new_start: f32) {
        self.data.resize_clip_start_to(clip_id, new_start, self.fps);
    }

    pub fn resize_clip_end_to(&mut self, clip_id: usize, new_end: f32) {
        self.data.resize_clip_end_to(clip_id, new_end, self.fps);
    }

    pub fn move_clip_to_track(&mut self, clip_id: usize, target_track_index: usize) {
        self.data
            .move_clip_to_track(clip_id, target_track_index, &mut self.interaction);
    }
}

fn snap_to_frame(time: f32, frame_duration: f32) -> f32 {
    if frame_duration <= 0.0 {
        return time.max(0.0);
    }
    (time / frame_duration).round() * frame_duration
}
