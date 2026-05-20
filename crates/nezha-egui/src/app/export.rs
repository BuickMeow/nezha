use super::App;
use eframe::egui;
use nezha_encoder::{
    Container, EncoderError, ExportConfig, FfmpegEncoder, QualityPreset, VideoCodec,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// 导出统计信息，用于 UI 展示。
#[derive(Debug, Clone)]
pub struct ExportStats {
    pub current_frame: u64,
    pub total_frames: u64,
    pub elapsed: Duration,
    pub current_fps: f64,
}

#[derive(Debug)]
pub enum ExportState {
    Exporting {
        encoder: FfmpegEncoder,
        current_frame: u64,
        total_frames: u64,
        started_at: Instant,
        /// 用于平滑 FPS 计算的滑动窗口
        last_stat_time: Instant,
        frames_since_stat: u64,
        smoothed_fps: f64,
    },
    Completed {
        total_frames: u64,
        elapsed: Duration,
        avg_fps: f64,
    },
    Error(String),
}

impl ExportState {
    pub fn stats(&self) -> Option<ExportStats> {
        match self {
            ExportState::Exporting {
                current_frame,
                total_frames,
                started_at,
                smoothed_fps,
                ..
            } => {
                let elapsed = started_at.elapsed();
                Some(ExportStats {
                    current_frame: *current_frame,
                    total_frames: *total_frames,
                    elapsed,
                    current_fps: *smoothed_fps,
                })
            }
            ExportState::Completed {
                total_frames,
                elapsed,
                avg_fps,
            } => Some(ExportStats {
                current_frame: *total_frames,
                total_frames: *total_frames,
                elapsed: *elapsed,
                current_fps: *avg_fps,
            }),
            ExportState::Error(_) => None,
        }
    }
}

impl App {
    pub(super) fn start_export(&mut self) {
        let path = match self.ui.export_path.as_ref() {
            Some(p) => PathBuf::from(p),
            None => {
                self.export_state = Some(ExportState::Error("未选择导出路径".to_string()));
                return;
            }
        };

        let container: Container = match self.ui.export_format.parse() {
            Ok(c) => c,
            Err(e) => {
                self.export_state = Some(ExportState::Error(e));
                return;
            }
        };

        let codec: VideoCodec = match self.ui.encoder.parse() {
            Ok(c) => c,
            Err(e) => {
                self.export_state = Some(ExportState::Error(e));
                return;
            }
        };

        let duration = self.project.duration();
        let fps = self.project.render.fps as f64;

        let config = ExportConfig {
            width: self.project.render.width,
            height: self.project.render.height,
            fps,
            container,
            codec,
            output_path: path,
            quality: QualityPreset::default(),
        };

        let total_frames = config.total_frames(duration);

        match FfmpegEncoder::new(&config) {
            Ok(encoder) => {
                self.project.playback.is_playing = false;
                let now = Instant::now();
                self.export_state = Some(ExportState::Exporting {
                    encoder,
                    current_frame: 0,
                    total_frames,
                    started_at: now,
                    last_stat_time: now,
                    frames_since_stat: 0,
                    smoothed_fps: 0.0,
                });
            }
            Err(EncoderError::FfmpegNotFound) => {
                let exe_name = if cfg!(target_os = "windows") {
                    "ffmpeg.exe"
                } else {
                    "ffmpeg"
                };
                self.export_state = Some(ExportState::Error(format!(
                    "未找到 {}。请在程序所在目录下放入对应平台的 ffmpeg 二进制文件。",
                    exe_name
                )));
            }
            Err(e) => {
                self.export_state = Some(ExportState::Error(format!("启动编码器失败: {}", e)));
            }
        }
    }

    /// 自适应时间预算批处理渲染。
    ///
    /// 每轮 UI 在 MAX_BATCH_DURATION 内尽可能多地渲染帧，
    /// 达到时间上限后主动 yield 给 UI 事件循环，
    /// 兼顾渲染吞吐量与 UI 响应速度。
    const MAX_BATCH_DURATION_MS: u64 = 20;

    /// FPS 统计刷新间隔
    const STAT_UPDATE_INTERVAL: Duration = Duration::from_millis(500);

    pub(super) fn export_step(&mut self) {
        match self.export_state.take() {
            Some(ExportState::Exporting {
                mut encoder,
                current_frame,
                total_frames,
                started_at,
                mut last_stat_time,
                mut frames_since_stat,
                mut smoothed_fps,
            }) => {
                if current_frame >= total_frames {
                    let elapsed = started_at.elapsed();
                    let avg_fps = if elapsed.as_secs_f64() > 0.0 {
                        total_frames as f64 / elapsed.as_secs_f64()
                    } else {
                        0.0
                    };
                    match encoder.finish() {
                        Ok(()) => {
                            self.export_state = Some(ExportState::Completed {
                                total_frames,
                                elapsed,
                                avg_fps,
                            });
                        }
                        Err(e) => {
                            self.export_state =
                                Some(ExportState::Error(format!("编码器收尾失败: {}", e)));
                        }
                    }
                    return;
                }

                let fps = self.project.render.fps as f64;
                let batch_deadline =
                    Instant::now() + Duration::from_millis(Self::MAX_BATCH_DURATION_MS);

                let mut frame = current_frame;

                loop {
                    if frame >= total_frames {
                        break;
                    }

                    let time = frame as f64 / fps;
                    let bytes = self.render_frame_combined(time as f32);
                    if bytes.is_empty() {
                        self.export_state =
                            Some(ExportState::Error("GPU 帧读取失败或超时".to_string()));
                        return;
                    }

                    match encoder.write_frame(bytes) {
                        Ok(()) => {
                            frame += 1;
                            frames_since_stat += 1;
                        }
                        Err(e) => {
                            self.export_state =
                                Some(ExportState::Error(format!("写入视频帧失败: {}", e)));
                            return;
                        }
                    }

                    // 定期刷新 FPS 统计（滑动平均）
                    let now = Instant::now();
                    let since_stat = now.duration_since(last_stat_time);
                    if since_stat >= Self::STAT_UPDATE_INTERVAL {
                        let instant_fps =
                            frames_since_stat as f64 / since_stat.as_secs_f64().max(0.001);
                        // 指数平滑：新值权重 0.4，旧值权重 0.6
                        if smoothed_fps == 0.0 {
                            smoothed_fps = instant_fps;
                        } else {
                            smoothed_fps = smoothed_fps * 0.6 + instant_fps * 0.4;
                        }
                        last_stat_time = now;
                        frames_since_stat = 0;
                    }

                    // 时间预算耗尽，yield 给 UI
                    if now >= batch_deadline {
                        break;
                    }
                }

                // 最终更新一次 FPS（避免小批次永远不刷新统计）
                if frames_since_stat > 0 {
                    let since_stat = Instant::now().duration_since(last_stat_time);
                    if since_stat.as_secs_f64() > 0.0 {
                        let instant_fps =
                            frames_since_stat as f64 / since_stat.as_secs_f64().max(0.001);
                        if smoothed_fps == 0.0 {
                            smoothed_fps = instant_fps;
                        } else {
                            smoothed_fps = smoothed_fps * 0.6 + instant_fps * 0.4;
                        }
                    }
                    last_stat_time = Instant::now();
                    frames_since_stat = 0;
                }

                self.export_state = Some(ExportState::Exporting {
                    encoder,
                    current_frame: frame,
                    total_frames,
                    started_at,
                    last_stat_time,
                    frames_since_stat,
                    smoothed_fps,
                });
            }
            other => {
                self.export_state = other;
            }
        }
    }

    pub(super) fn show_export_overlay(&mut self, ui: &mut egui::Ui) {
        let mut dismiss = false;
        if let Some(status) = &self.export_state {
            let screen_rect = ui.ctx().content_rect();
            ui.ctx()
                .layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    "export_overlay".into(),
                ))
                .rect_filled(
                    screen_rect,
                    0.0,
                    egui::Color32::from_rgba_premultiplied(0, 0, 0, 160),
                );

            egui::Window::new("导出视频")
                .order(egui::Order::Tooltip)
                .collapsible(false)
                .resizable(false)
                .movable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| match status {
                    ExportState::Exporting { .. } => {
                        if let Some(stats) = status.stats() {
                            let progress =
                                stats.current_frame as f32 / stats.total_frames.max(1) as f32;

                            // 帧进度
                            ui.label(format!(
                                "帧: {} / {}",
                                stats.current_frame, stats.total_frames
                            ));

                            // 时间进度
                            let fps = self.project.render.fps as f64;
                            let current_secs = stats.current_frame as f64 / fps.max(1.0);
                            let total_secs = stats.total_frames as f64 / fps.max(1.0);
                            ui.label(format!(
                                "时间: {} / {}",
                                format_duration(current_secs),
                                format_duration(total_secs)
                            ));

                            ui.add(egui::ProgressBar::new(progress).show_percentage());

                            ui.separator();

                            // 实时渲染速度
                            ui.label(format!("渲染速度: {:.0} fps", stats.current_fps));

                            // 速度倍率
                            let speed = if stats.elapsed.as_secs_f64() > 0.0 && fps > 0.0 {
                                let rendered_duration = stats.current_frame as f64 / fps;
                                rendered_duration / stats.elapsed.as_secs_f64()
                            } else {
                                0.0
                            };
                            ui.label(format!("速度: {:.1}x 原速", speed));

                            // 已用时间 / 预估剩余
                            let elapsed = stats.elapsed;
                            if stats.current_frame > 0 && stats.current_fps > 0.0 {
                                let remaining_frames = stats.total_frames - stats.current_frame;
                                let remaining_secs = remaining_frames as f64 / stats.current_fps;
                                ui.label(format!(
                                    "已用: {} / 剩余: {}",
                                    format_duration(elapsed.as_secs_f64()),
                                    format_duration(remaining_secs)
                                ));
                            } else {
                                ui.label(format!(
                                    "已用: {}",
                                    format_duration(elapsed.as_secs_f64())
                                ));
                            }
                        }

                        if ui.button("取消").clicked() {
                            dismiss = true;
                        }
                    }
                    ExportState::Completed {
                        total_frames,
                        elapsed,
                        avg_fps,
                    } => {
                        ui.label("✅ 导出完成！");
                        ui.separator();
                        ui.label(format!("总帧数: {}", total_frames));
                        let total_secs = *total_frames as f64 / self.project.render.fps as f64;
                        ui.label(format!("时长: {}", format_duration(total_secs)));
                        ui.label(format!(
                            "总用时: {}",
                            format_duration(elapsed.as_secs_f64())
                        ));
                        ui.label(format!("平均速度: {:.0} fps", avg_fps));
                        if *elapsed > Duration::ZERO && total_secs > 0.0 {
                            ui.label(format!(
                                "倍率: {:.1}x 原速",
                                total_secs / elapsed.as_secs_f64()
                            ));
                        }
                        if ui.button("确定").clicked() {
                            dismiss = true;
                        }
                    }
                    ExportState::Error(msg) => {
                        ui.label("❌ 导出失败");
                        ui.label(
                            egui::RichText::new(msg.as_str())
                                .color(egui::Color32::from_rgb(255, 100, 100)),
                        );
                        if ui.button("确定").clicked() {
                            dismiss = true;
                        }
                    }
                });
        }
        if dismiss {
            self.export_state = None;
        }
    }
}

/// 格式化秒数为 mm:ss.s 或 hh:mm:ss
fn format_duration(secs: f64) -> String {
    if secs <= 0.0 {
        return "0:00.0".to_string();
    }
    let total_secs = secs;
    let hours = (total_secs / 3600.0) as u32;
    let minutes = (total_secs / 60.0) as u32 % 60;
    let seconds = total_secs % 60.0;
    if hours > 0 {
        format!("{}:{:02}:{:04.1}", hours, minutes, seconds)
    } else {
        format!("{}:{:04.1}", minutes, seconds)
    }
}
