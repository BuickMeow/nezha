use super::TimelineState;

pub enum TrackEditCommand {
    SelectClip(usize),
    ClearSelection,
    MoveClipToStart { clip_id: usize, start: f32 },
    ResizeClipStartTo { clip_id: usize, start: f32 },
    ResizeClipEndTo { clip_id: usize, end: f32 },
    MoveClipToTrack { clip_id: usize, target_track_index: usize },
}

pub fn apply_track_commands(state: &mut TimelineState, commands: Vec<TrackEditCommand>) {
    for command in commands {
        match command {
            TrackEditCommand::SelectClip(clip_id) => state.select_clip(clip_id),
            TrackEditCommand::ClearSelection => state.clear_selection(),
            TrackEditCommand::MoveClipToStart { clip_id, start } => {
                state.move_clip_to_start(clip_id, start);
            }
            TrackEditCommand::ResizeClipStartTo { clip_id, start } => {
                state.resize_clip_start_to(clip_id, start);
            }
            TrackEditCommand::ResizeClipEndTo { clip_id, end } => {
                state.resize_clip_end_to(clip_id, end);
            }
            TrackEditCommand::MoveClipToTrack {
                clip_id,
                target_track_index,
            } => {
                state.move_clip_to_track(clip_id, target_track_index);
            }
        }
    }
}
