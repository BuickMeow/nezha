use super::model::TimelineState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SnapKind {
    /// clip 容器边缘 (start / end)
    ClipEdge,
    /// clip 内容边缘 (content_start_time / content_end_time)
    ContentEdge,
    /// 播放头位置
    Playhead,
    /// 时间线起点 0.0
    TimelineStart,
}

#[derive(Clone, Copy, Debug)]
pub struct SnapPoint {
    pub time: f32,
    pub kind: SnapKind,
}

#[derive(Clone, Copy, Debug)]
pub struct SnapResult {
    pub snapped_time: f32,
    pub snap_point: SnapPoint,
}

/// 收集时间线上所有吸附点，排除指定 clip 的所有边缘。
pub fn collect_snap_points(state: &TimelineState, current_time: f32, exclude_clip_id: Option<usize>) -> Vec<SnapPoint> {
    let mut points = Vec::new();

    // 时间线起点
    points.push(SnapPoint { time: 0.0, kind: SnapKind::TimelineStart });

    // 播放头
    if current_time > 0.0 {
        points.push(SnapPoint { time: current_time, kind: SnapKind::Playhead });
    }

    let fps = state.fps;
    for track in &state.data.tracks {
        for clip in &track.clips {
            if Some(clip.id) == exclude_clip_id {
                continue;
            }
            // 容器边缘
            points.push(SnapPoint { time: clip.start, kind: SnapKind::ClipEdge });
            points.push(SnapPoint { time: clip.end, kind: SnapKind::ClipEdge });
            // 内容边缘（仅当有 offset 时才与容器边缘不同）
            if clip.content_start_offset > 0 || clip.content_end_offset > 0 {
                points.push(SnapPoint { time: clip.content_start_time(fps), kind: SnapKind::ContentEdge });
                points.push(SnapPoint { time: clip.content_end_time(fps), kind: SnapKind::ContentEdge });
            }
        }
    }

    points
}

/// 在阈值内查找最近的吸附点。
pub fn find_snap(candidate_time: f32, snap_points: &[SnapPoint], threshold: f32) -> Option<SnapResult> {
    let mut best: Option<SnapResult> = None;
    let mut best_dist = threshold;

    for point in snap_points {
        let dist = (candidate_time - point.time).abs();
        if dist < best_dist {
            best_dist = dist;
            best = Some(SnapResult {
                snapped_time: point.time,
                snap_point: *point,
            });
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_snap_within_threshold() {
        let points = vec![
            SnapPoint { time: 1.0, kind: SnapKind::ClipEdge },
            SnapPoint { time: 5.0, kind: SnapKind::ClipEdge },
        ];
        // 候选时间 1.05，距离 1.0 只有 0.05
        let result = find_snap(1.05, &points, 0.1);
        assert!(result.is_some());
        let snap = result.unwrap();
        assert!((snap.snapped_time - 1.0).abs() < 0.001);
        assert_eq!(snap.snap_point.kind, SnapKind::ClipEdge);
    }

    #[test]
    fn test_find_snap_outside_threshold() {
        let points = vec![
            SnapPoint { time: 1.0, kind: SnapKind::ClipEdge },
        ];
        // 距离 0.5，超过阈值 0.1
        let result = find_snap(1.5, &points, 0.1);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_snap_nearest_wins() {
        let points = vec![
            SnapPoint { time: 2.0, kind: SnapKind::Playhead },
            SnapPoint { time: 2.05, kind: SnapKind::ClipEdge },
        ];
        // 候选 2.03，距离 ClipEdge 0.02，距离 Playhead 0.03
        let result = find_snap(2.03, &points, 0.1);
        assert!(result.is_some());
        let snap = result.unwrap();
        assert_eq!(snap.snap_point.kind, SnapKind::ClipEdge);
    }

    #[test]
    fn test_find_snap_empty_points() {
        let result = find_snap(1.0, &[], 0.1);
        assert!(result.is_none());
    }

    #[test]
    fn test_collect_snap_points_excludes_clip() {
        let mut state = TimelineState::default();
        // id1 duration=5.0 → end=5.0; id2 duration=3.0 → end=3.0
        let id1 = state.data.push_solid_color_clip(egui::Color32::RED, 5.0);
        let _id2 = state.data.push_solid_color_clip(egui::Color32::BLUE, 3.0);

        // 排除 id1：id1 的 end=5.0 不应出现
        let points = collect_snap_points(&state, 0.0, Some(id1));
        let has_id1_end = points.iter().any(|p| (p.time - 5.0).abs() < 0.001 && p.kind == SnapKind::ClipEdge);
        assert!(!has_id1_end, "excluded clip's end edge should not appear");

        // id2 的 end=3.0 仍应存在
        let has_id2_end = points.iter().any(|p| (p.time - 3.0).abs() < 0.001 && p.kind == SnapKind::ClipEdge);
        assert!(has_id2_end, "non-excluded clip's end edge should still appear");

        // 不排除任何 clip：两个 clip 各 2 个 ClipEdge = 4
        let points_all = collect_snap_points(&state, 0.0, None);
        let clip_edges: Vec<_> = points_all.iter().filter(|p| p.kind == SnapKind::ClipEdge).collect();
        assert_eq!(clip_edges.len(), 4);
    }
}
