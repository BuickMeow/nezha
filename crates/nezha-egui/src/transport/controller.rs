use std::collections::HashSet;

use super::interaction::{BoxSelect, BoxSelectMode};
use super::{ClipDragState, ScrollbarDrag, TimelineState, TrackKind};

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
    ToggleClipSelection(usize),
    AddToSelection(usize),
    SelectClips(HashSet<usize>),
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
        target_track_kind: TrackKind,
    },
    ToggleTrackMute(usize),
    ToggleTrackHidden(usize),
    ToggleTrackLocked(usize),
    SetSnapLine(Option<f32>),
    StartBoxSelect(f32, f32, BoxSelectMode),
    UpdateBoxSelect(f32, f32),
    FinishBoxSelect(HashSet<usize>),
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
            TimelineCommand::SelectClip(clip_id) => state.selection.select(clip_id),
            TimelineCommand::ToggleClipSelection(clip_id) => state.selection.toggle(clip_id),
            TimelineCommand::AddToSelection(clip_id) => state.selection.add(clip_id),
            TimelineCommand::SelectClips(ids) => state.selection.set_bulk(ids),
            TimelineCommand::ClearSelection => state.selection.clear(),
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
                target_track_kind,
            } => {
                state.move_clip_to_track(clip_id, target_track_index, target_track_kind);
            }
            TimelineCommand::ToggleTrackMute(track_index) => {
                state.data.toggle_track_mute(track_index);
            }
            TimelineCommand::ToggleTrackHidden(track_index) => {
                state.data.toggle_track_hidden(track_index);
            }
            TimelineCommand::ToggleTrackLocked(track_index) => {
                state.data.toggle_track_locked(track_index);
            }
            TimelineCommand::SetSnapLine(pos) => {
                state.interaction.snap_line = pos;
            }
            TimelineCommand::StartBoxSelect(start_x, start_y, mode) => {
                state.interaction.box_select = Some(BoxSelect {
                    start_x,
                    start_y,
                    current_x: start_x,
                    current_y: start_y,
                    mode,
                });
            }
            TimelineCommand::UpdateBoxSelect(x, y) => {
                if let Some(bs) = &mut state.interaction.box_select {
                    bs.current_x = x;
                    bs.current_y = y;
                }
            }
            TimelineCommand::FinishBoxSelect(hit_ids) => {
                if let Some(bs) = &state.interaction.box_select {
                    match bs.mode {
                        BoxSelectMode::Replace => {
                            state.selection.set_bulk(hit_ids);
                        }
                        BoxSelectMode::Add => {
                            for id in &hit_ids {
                                state.selection.add(*id);
                            }
                        }
                        BoxSelectMode::Toggle => {
                            for id in &hit_ids {
                                state.selection.toggle(*id);
                            }
                        }
                    }
                }
                state.interaction.box_select = None;
            }
        }
    }
}
