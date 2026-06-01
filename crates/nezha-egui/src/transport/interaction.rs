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
    pub track_was_inserted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BoxSelectMode {
    /// 无修饰键：替换选择
    Replace,
    /// Shift：追加到选择
    Add,
    /// Ctrl/Cmd：切换每个 clip 的选择状态
    Toggle,
}

#[derive(Clone, Debug)]
pub struct BoxSelect {
    /// 框选起点（屏幕坐标）
    pub start_x: f32,
    pub start_y: f32,
    /// 当前拖拽位置（屏幕坐标）
    pub current_x: f32,
    pub current_y: f32,
    pub mode: BoxSelectMode,
}

#[derive(Clone, Debug, Default)]
pub struct TimelineInteraction {
    pub dragging_playhead: bool,
    pub scrollbar_drag: Option<ScrollbarDrag>,
    pub clip_drag: Option<ClipDragState>,
    /// 当前帧吸附指示线的时间位置，None 表示无吸附
    pub snap_line: Option<f32>,
    /// 框选状态
    pub box_select: Option<BoxSelect>,
}
