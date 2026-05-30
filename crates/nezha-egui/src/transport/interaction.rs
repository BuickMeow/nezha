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

#[derive(Clone, Debug, Default)]
pub struct TimelineInteraction {
    pub dragging_playhead: bool,
    pub scrollbar_drag: Option<ScrollbarDrag>,
    pub clip_drag: Option<ClipDragState>,
}
