use super::TimelineState;

pub enum TrackEditCommand {
    SelectClip(usize),
    ClearSelection,
    MoveClip { clip_id: usize, delta: f32 },
    ResizeClipStart { clip_id: usize, delta: f32 },
    ResizeClipEnd { clip_id: usize, delta: f32 },
    MoveClipToTrack { clip_id: usize, target_track_index: usize },
}

pub fn apply_track_commands(state: &mut TimelineState, commands: Vec<TrackEditCommand>) {
    for command in commands {
        match command {
            TrackEditCommand::SelectClip(clip_id) => state.select_clip(clip_id),
            TrackEditCommand::ClearSelection => state.clear_selection(),
            TrackEditCommand::MoveClip { clip_id, delta } => state.move_clip_by(clip_id, delta),
            TrackEditCommand::ResizeClipStart { clip_id, delta } => {
                state.resize_clip_start_by(clip_id, delta);
            }
            TrackEditCommand::ResizeClipEnd { clip_id, delta } => {
                state.resize_clip_end_by(clip_id, delta);
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
