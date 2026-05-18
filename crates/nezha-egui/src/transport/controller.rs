use super::{ClipDragState, ScrollbarDrag, TimelineState};

pub enum TimelineCommand {
    SetPlaying(bool),
    StopPlayback,
    SetCurrentTime(f32),
    SetScrollbarDrag(Option<ScrollbarDrag>),
    SetPlayheadDragging(bool),
    SetClipDrag(Option<ClipDragState>),
    SetScrollOffset(f32),
    SetZoomAndScroll {
        zoom: f32,
        scroll_offset: f32,
    },
    SelectClip(usize),
    ClearSelection,
    MoveClipToStart {
        clip_id: usize,
        start: f32,
    },
    ResizeClipStartTo {
        clip_id: usize,
        start: f32,
    },
    ResizeClipEndTo {
        clip_id: usize,
        end: f32,
    },
    MoveClipToTrack {
        clip_id: usize,
        target_track_index: usize,
    },
}

pub fn apply_timeline_commands(
    is_playing: &mut bool,
    current_time: &mut f32,
    state: &mut TimelineState,
    commands: Vec<TimelineCommand>,
) {
    for command in commands {
        match command {
            TimelineCommand::SetPlaying(value) => *is_playing = value,
            TimelineCommand::StopPlayback => {
                *is_playing = false;
                *current_time = 0.0;
            }
            TimelineCommand::SetCurrentTime(time) => *current_time = time,
            TimelineCommand::SetScrollbarDrag(drag) => state.interaction.scrollbar_drag = drag,
            TimelineCommand::SetPlayheadDragging(dragging) => {
                state.interaction.dragging_playhead = dragging;
            }
            TimelineCommand::SetClipDrag(drag) => state.interaction.clip_drag = drag,
            TimelineCommand::SetScrollOffset(offset) => {
                state.view.scroll_offset = offset.max(0.0);
            }
            TimelineCommand::SetZoomAndScroll {
                zoom,
                scroll_offset,
            } => {
                state.view.zoom = zoom.clamp(0.2, 5000.0);
                state.view.scroll_offset = scroll_offset.max(0.0);
            }
            TimelineCommand::SelectClip(clip_id) => state.select_clip(clip_id),
            TimelineCommand::ClearSelection => state.clear_selection(),
            TimelineCommand::MoveClipToStart { clip_id, start } => {
                state.move_clip_to_start(clip_id, start);
            }
            TimelineCommand::ResizeClipStartTo { clip_id, start } => {
                state.resize_clip_start_to(clip_id, start);
            }
            TimelineCommand::ResizeClipEndTo { clip_id, end } => {
                state.resize_clip_end_to(clip_id, end);
            }
            TimelineCommand::MoveClipToTrack {
                clip_id,
                target_track_index,
            } => {
                state.move_clip_to_track(clip_id, target_track_index);
            }
        }
    }
}
