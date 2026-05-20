use super::App;
use eframe::egui;
use nezha_encoder::{
    Container, EncoderError, ExportConfig, FfmpegEncoder, QualityPreset, VideoCodec,
};
use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;
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
        /// 已提交 GPU 渲染的帧序号
        rendered_frame: u64,
        /// 已写入 ffmpeg 的帧序号
        written_frame: u64,
        total_frames: u64,
        started_at: Instant,
        last_stat_time: Instant,
        frames_since_stat: u64,
        smoothed_fps: f64,
    },
    /// ffmpeg 正在后台收尾（不再渲染新帧）
    Finalizing {
        started_at: Instant,
        total_frames: u64,
        written_frame: u64,
        smoothed_fps: f64,
        finish_rx: std::sync::mpsc::Receiver<Result<(), EncoderError>>,
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
                written_frame,
                total_frames,
                started_at,
                smoothed_fps,
                ..
            } => {
                let elapsed = started_at.elapsed();
                Some(ExportStats {
                    current_frame: *written_frame,
                    total_frames: *total_frames,
                    elapsed,
                    current_fps: *smoothed_fps,
                })
            }
            ExportState::Finalizing {
                total_frames,
                written_frame,
                started_at,
                smoothed_fps,
                ..
            } => {
                let elapsed = started_at.elapsed();
                Some(ExportStats {
                    current_frame: *written_frame,
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
                    rendered_frame: 0,
                    written_frame: 0,
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

    /// 自适应时间预算 + 流水线渲染。
    ///
    /// GPU 渲染和 CPU 读回通过 triple buffering 流水线并行：
    ///   - render_frame_pipelined() 提交 GPU 工作后立即返回
    ///   - try_read_staging() 非阻塞读取已完成的帧
    ///   - 当 ring 满（3 帧在飞行中）时，必须 wait_read_staging() 释放槽位
    const MAX_BATCH_DURATION_MS: u64 = 20;
    const STAT_UPDATE_INTERVAL: Duration = Duration::from_millis(500);

    pub(super) fn export_step(&mut self) {
        match self.export_state.take() {
            Some(ExportState::Exporting {
                mut encoder,
                mut rendered_frame,
                mut written_frame,
                total_frames,
                started_at,
                mut last_stat_time,
                mut frames_since_stat,
                mut smoothed_fps,
            }) => {
                let fps = self.project.render.fps as f64;
                let batch_deadline =
                    Instant::now() + Duration::from_millis(Self::MAX_BATCH_DURATION_MS);

                loop {
                    // —— 阶段 1：尝试读回已完成的帧并写入 ffmpeg ——
                    while let Some(data) = self.render_ctx.try_read_staging() {
                        match encoder.write_frame(data) {
                            Ok(()) => {
                                written_frame += 1;
                                frames_since_stat += 1;
                            }
                            Err(e) => {
                                self.export_state =
                                    Some(ExportState::Error(format!("写入视频帧失败: {}", e)));
                                return;
                            }
                        }
                    }

                    // —— 阶段 2：渲染新帧（如果有剩余帧且 ring 有空位） ——
                    if rendered_frame < total_frames && self.render_ctx.staging_can_write() {
                        let time = rendered_frame as f64 / fps;
                        self.render_frame_pipelined(time as f32);
                        rendered_frame += 1;
                        continue; // 尝试继续渲染/读回
                    }

                    // —— 阶段 3：判断终止条件 ——
                    let all_rendered = rendered_frame >= total_frames;
                    let all_written = written_frame >= total_frames;

                    if all_rendered && all_written {
                        // 所有帧已提交且已写入，转入后台 Finalizing
                        let (tx, rx) = std::sync::mpsc::channel();
                        std::thread::spawn(move || {
                            let result = encoder.finish();
                            let _ = tx.send(result);
                        });
                        self.export_state = Some(ExportState::Finalizing {
                            started_at,
                            total_frames,
                            written_frame,
                            smoothed_fps,
                            finish_rx: rx,
                        });
                        return;
                    }

                    if all_rendered && self.render_ctx.staging_has_pending() {
                        // 所有帧已提交，还有未读回的：yield 给 UI，GPU 正在完成
                        break;
                    }

                    // —— 阶段 4：FPS 统计（每 500ms） ——
                    let now = Instant::now();
                    let since_stat = now.duration_since(last_stat_time);
                    if since_stat >= Self::STAT_UPDATE_INTERVAL {
                        let instant_fps =
                            frames_since_stat as f64 / since_stat.as_secs_f64().max(0.001);
                        if smoothed_fps == 0.0 {
                            smoothed_fps = instant_fps;
                        } else {
                            smoothed_fps = smoothed_fps * 0.6 + instant_fps * 0.4;
                        }
                        last_stat_time = now;
                        frames_since_stat = 0;
                    }

                    // —— 阶段 5：时间预算耗尽，yield 给 UI ——
                    if now >= batch_deadline {
                        break;
                    }

                    // —— 阶段 6：无法继续推进 ——
                    // ring 满且无就绪数据（GPU 还在处理最早提交的帧）
                    // → yield 给 UI，让 GPU 有时间完成
                    break;
                }

                // 批次结束前最后一次 FPS 更新
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
                }

                self.export_state = Some(ExportState::Exporting {
                    encoder,
                    rendered_frame,
                    written_frame,
                    total_frames,
                    started_at,
                    last_stat_time: Instant::now(),
                    frames_since_stat: 0,
                    smoothed_fps,
                });
            }

            // —— Finalizing：等待后台 ffmpeg 收尾完成 ——
            Some(ExportState::Finalizing {
                started_at,
                total_frames,
                written_frame,
                smoothed_fps,
                finish_rx,
            }) => {
                match finish_rx.try_recv() {
                    Ok(Ok(())) => {
                        let elapsed = started_at.elapsed();
                        let avg_fps = if elapsed.as_secs_f64() > 0.0 {
                            total_frames as f64 / elapsed.as_secs_f64()
                        } else {
                            0.0
                        };
                        self.export_state = Some(ExportState::Completed {
                            total_frames,
                            elapsed,
                            avg_fps,
                        });
                    }
                    Ok(Err(e)) => {
                        self.export_state =
                            Some(ExportState::Error(format!("编码器收尾失败: {}", e)));
                    }
                    Err(TryRecvError::Empty) => {
                        // 仍在处理中
                        self.export_state = Some(ExportState::Finalizing {
                            started_at,
                            total_frames,
                            written_frame,
                            smoothed_fps,
                            finish_rx,
                        });
                    }
                    Err(TryRecvError::Disconnected) => {
                        self.export_state =
                            Some(ExportState::Error("编码器线程异常退出".to_string()));
                    }
                }
            }

            other => {
                self.export_state = other;
            }
        }
    }

    pub(super) fn show_export_overlay(&mut self, ui: &mut egui::Ui) {
        let mut dismiss = false;
        let mut force_finish = false;
        // 提前提取 Finalizing 的统计信息（避免 borrow 冲突）
        let finalizing_stats = match &self.export_state {
            Some(ExportState::Finalizing { .. }) => {
                self.export_state.as_ref().and_then(|s| s.stats())
            }
            _ => None,
        };

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
                            Self::render_export_progress(ui, &stats, self.project.render.fps);
                        }
                        if ui.button("取消").clicked() {
                            dismiss = true;
                        }
                    }
                    ExportState::Finalizing { .. } => {
                        if let Some(stats) = &finalizing_stats {
                            ui.label("⏳ 正在完成编码...");
                            ui.separator();
                            Self::render_export_progress(ui, stats, self.project.render.fps);
                            ui.separator();
                            ui.label("(ffmpeg 正在封装文件，请稍候)");
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button("✅ 强制完成").clicked() {
                                    force_finish = true;
                                }
                                ui.label("视频已可用，跳过等待");
                            });
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
        if force_finish {
            if let Some(stats) = finalizing_stats {
                self.export_state = Some(ExportState::Completed {
                    total_frames: stats.total_frames,
                    elapsed: stats.elapsed,
                    avg_fps: stats.current_fps,
                });
            }
        } else if dismiss {
            self.export_state = None;
        }
    }

    fn render_export_progress(ui: &mut egui::Ui, stats: &ExportStats, fps: u32) {
        let progress = stats.current_frame as f32 / stats.total_frames.max(1) as f32;
        let fps_f = fps as f64;

        ui.label(format!(
            "帧: {} / {}",
            stats.current_frame, stats.total_frames
        ));

        let current_secs = stats.current_frame as f64 / fps_f.max(1.0);
        let total_secs = stats.total_frames as f64 / fps_f.max(1.0);
        ui.label(format!(
            "时间: {} / {}",
            format_duration(current_secs),
            format_duration(total_secs)
        ));

        ui.add(egui::ProgressBar::new(progress).show_percentage());

        ui.separator();

        ui.label(format!("渲染速度: {:.0} fps", stats.current_fps));

        let speed = if stats.elapsed.as_secs_f64() > 0.0 && fps_f > 0.0 {
            let rendered_duration = stats.current_frame as f64 / fps_f;
            rendered_duration / stats.elapsed.as_secs_f64()
        } else {
            0.0
        };
        ui.label(format!("速度: {:.1}x 原速", speed));

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
            ui.label(format!("已用: {}", format_duration(elapsed.as_secs_f64())));
        }
    }
}

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
