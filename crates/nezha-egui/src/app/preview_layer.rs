use crate::transport::{ClipKind, LayerCommon, TrackClip};
use nezha_renderer::RenderMode;

/// 图层渲染所需数据（从 [`TrackClip`] 拷贝，避免生命周期问题）。
#[derive(Clone)]
pub(super) struct LayerData {
    pub clip_id: usize,
    pub kind: ClipKind,
    pub midi_idx: Option<usize>,
    pub speed: f32,
    pub border_width: f32,
    pub rounding: f32,
    pub render_mode: RenderMode,
    pub equal_key_width: bool,
    pub clip_start: f32,
    pub color: egui::Color32,
    pub text_color: egui::Color32,
    pub keyboard_height_percent: f32,
    pub font_size: u32,
    pub common: LayerCommon,
}

impl From<&TrackClip> for LayerData {
    fn from(clip: &TrackClip) -> Self {
        Self {
            clip_id: clip.id,
            kind: clip.kind,
            midi_idx: clip.midi_idx,
            speed: clip.speed,
            border_width: clip.border_width,
            rounding: clip.rounding,
            render_mode: clip.render_mode,
            equal_key_width: clip.equal_key_width,
            clip_start: clip.start,
            color: clip.color,
            text_color: clip.text_color,
            keyboard_height_percent: clip.keyboard_height_percent,
            font_size: clip.font_size,
            common: clip.common.clone(),
        }
    }
}

/// 收集当前时间点所有可见图层数据（Premiere 顺序：底 -> 顶）。
pub(super) fn collect_visible_layers(
    tracks: &[crate::transport::Track],
    time: f32,
) -> Vec<LayerData> {
    let mut layers = Vec::new();
    for track in tracks.iter().rev() {
        for clip in &track.clips {
            if time >= clip.start && time < clip.end {
                layers.push(LayerData::from(clip));
            }
        }
    }
    layers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{ClipKind, Track, TrackClip};

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
        let mut track = Track::new_video("test");
        track.clips.push(make_waterfall_clip(1, 0.0, 10.0));
        let layers = collect_visible_layers(&[track], 5.0);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].clip_id, 1);
    }

    #[test]
    fn test_collect_visible_layers_outside_range() {
        let mut track = Track::new_video("test");
        track.clips.push(make_waterfall_clip(1, 0.0, 10.0));
        let layers = collect_visible_layers(&[track], 15.0);
        assert!(layers.is_empty(), "clip should not be visible at time 15");
    }

    #[test]
    fn test_collect_visible_layers_reverse_order() {
        let mut track = Track::new_video("test");
        track.clips.push(make_waterfall_clip(1, 0.0, 10.0));
        track.clips.push(make_waterfall_clip(2, 0.0, 10.0));
        let layers = collect_visible_layers(&[track], 5.0);
        assert_eq!(layers.len(), 2);
        // First clip is bottom (background), rendered first
        assert_eq!(layers[0].clip_id, 1);
        assert_eq!(layers[1].clip_id, 2);
    }

    #[test]
    fn test_layer_data_from_track_clip() {
        let clip = make_waterfall_clip(42, 1.0, 5.0);
        let data = LayerData::from(&clip);
        assert_eq!(data.clip_id, 42);
        assert_eq!(data.clip_start, 1.0);
        assert_eq!(data.kind, ClipKind::Waterfall);
    }
}
