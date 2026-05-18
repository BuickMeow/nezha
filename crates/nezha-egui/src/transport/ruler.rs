use eframe::egui;
use crate::transport::layout::{TimelineLayout, TimelineMetrics};
use crate::transport::{TimelineState, ThemeColors};
use crate::transport::timecode::{snap_to_frame, format_timecode_frames, format_timecode_seconds, font};

pub fn draw_ruler(
    ui: &egui::Ui,
    painter: &egui::Painter,
    c: &ThemeColors,
    layout: &TimelineLayout,
    _metrics: &TimelineMetrics,
    state: &mut TimelineState,
    response: &egui::Response,
    current_time: &mut f32,
    duration: f32,
    fps: u32,
) {
    let timeline_rect = layout.timeline_rect;
    let ruler_rect = layout.ruler_rect;
    let visible_start = layout.visible_start;
    let visible_end = layout.visible_end;
    painter.rect_filled(ruler_rect, 0.0, c.ruler_bg);
    painter.rect_stroke(ruler_rect, 0.0, egui::Stroke::new(1.0, c.border), egui::StrokeKind::Inside);

    // 点击 ruler 跳转
    if response.clicked_by(egui::PointerButton::Primary)
        && !ui.input(|i| i.modifiers.shift)
        && !state.interaction.dragging_playhead
        && state.interaction.scrollbar_drag.is_none()
    {
        if let Some(mouse_pos) = response.hover_pos() {
            if ruler_rect.contains(mouse_pos) && mouse_pos.x > timeline_rect.min.x + state.view.header_width {
                let new_time = state.view.time_at_screen_x(&timeline_rect, mouse_pos.x);
                *current_time = snap_to_frame(new_time, fps).clamp(0.0, duration);
            }
        }
    }

    let frame_interval = 1.0 / fps.max(1) as f32;

    let thresholds = [
        (5000.0, frame_interval),
        (3000.0, 2.0 * frame_interval),
        (1500.0, 5.0 * frame_interval),
        (750.0, 10.0 * frame_interval),
        (300.0, 30.0 * frame_interval),
        (150.0, 60.0 * frame_interval),
        (100.0, 2.0),
        (50.0, 5.0),
        (20.0, 10.0),
        (10.0, 30.0),
        (5.0, 60.0),
        (2.0, 120.0),
    ];
    let major_interval = thresholds
        .iter()
        .find(|(z, _)| state.view.zoom > *z)
        .map(|(_, interval)| *interval)
        .unwrap_or(300.0);

    // 主刻度
    let mut t = (visible_start / major_interval).floor() * major_interval;
    while t <= visible_end {
        let x = state.view.screen_x_for_time(&timeline_rect, t);
        if x >= timeline_rect.min.x + state.view.header_width {
            painter.line_segment(
                [
                    egui::pos2(x, ruler_rect.min.y + 14.0),
                    egui::pos2(x, ruler_rect.max.y),
                ],
                egui::Stroke::new(1.0, c.ruler_tick),
            );
            let label = if major_interval < 1.0 {
                format_timecode_frames(t, fps)
            } else {
                format_timecode_seconds(t)
            };
            painter.text(
                egui::pos2(x + 3.0, ruler_rect.min.y + 2.0),
                egui::Align2::LEFT_TOP,
                label,
                font(11.0),
                c.ruler_text,
            );
        }
        t += major_interval;
    }

    // 次刻度（帧级别）
    if state.view.zoom > 3000.0 {
        let mut ft = (visible_start / frame_interval).floor() * frame_interval;
        while ft <= visible_end {
            let x = state.view.screen_x_for_time(&timeline_rect, ft);
            if x >= timeline_rect.min.x + state.view.header_width {
                let is_major = ((ft / major_interval).round() * major_interval - ft).abs() < 0.001;
                if !is_major {
                    painter.line_segment(
                        [
                            egui::pos2(x, ruler_rect.min.y + 20.0),
                            egui::pos2(x, ruler_rect.max.y),
                        ],
                        egui::Stroke::new(1.0, c.ruler_tick.gamma_multiply(0.5)),
                    );
                }
            }
            ft += frame_interval;
        }
    }
}
