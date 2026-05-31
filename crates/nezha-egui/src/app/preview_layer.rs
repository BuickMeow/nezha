use crate::transport::TrackClip;

pub(super) fn collect_visible_layers<'a>(
    tracks: &'a [crate::transport::Track],
    time: f32,
) -> Vec<&'a TrackClip> {
    let mut layers = Vec::new();
    for track in tracks.iter().rev() {
        if track.hidden {
            continue;
        }
        for clip in &track.clips {
            if time >= clip.start && time < clip.end {
                layers.push(clip);
            }
        }
    }
    layers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{Track, TrackClip, TrackKind};

    fn make_waterfall_clip(id: usize, start: f32, end: f32) -> TrackClip {
        let mut clip = TrackClip::new_waterfall(id, None);
        clip.start = start;
        clip.end = end;
        clip
    }

    #[test]
    fn test_collect_visible_layers_empty() {
        let layers = collect_visible_layers(&[], 10.0);
        assert!(layers.is_empty());
    }

    #[test]
    fn test_collect_visible_layers_single() {
        let mut track = Track::new("test", TrackKind::Video);
        track.clips.push(make_waterfall_clip(1, 0.0, 10.0));
        let tracks = [track];
        let layers = collect_visible_layers(&tracks, 5.0);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].id, 1);
    }

    #[test]
    fn test_collect_visible_layers_outside_range() {
        let mut track = Track::new("test", TrackKind::Video);
        track.clips.push(make_waterfall_clip(1, 0.0, 10.0));
        let tracks = [track];
        let layers = collect_visible_layers(&tracks, 15.0);
        assert!(layers.is_empty(), "clip should not be visible at time 15");
    }

    #[test]
    fn test_collect_visible_layers_reverse_order() {
        let mut track = Track::new("test", TrackKind::Video);
        track.clips.push(make_waterfall_clip(1, 0.0, 10.0));
        track.clips.push(make_waterfall_clip(2, 0.0, 10.0));
        let tracks = [track];
        let layers = collect_visible_layers(&tracks, 5.0);
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].id, 1);
        assert_eq!(layers[1].id, 2);
    }
}
