use eframe::egui;
use nezha_encoder::{EncoderError, FfmpegEncoder};
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
        rendered_frame: u64,
        written_frame: u64,
        total_frames: u64,
        started_at: Instant,
        last_stat_time: Instant,
        frames_since_stat: u64,
        smoothed_fps: f64,
    },
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

pub(crate) struct ExportController {
    pub state: Option<ExportState>,
}

impl ExportController {
    pub fn new() -> Self {
        Self { state: None }
    }

    pub fn has_export(&self) -> bool {
        self.state.is_some()
    }

    pub fn start(
        &mut self,
        export_path: Option<&str>,
        export_format: &str,
        encoder: &str,
        encoder_backend: &str,
        duration: f64,
        render: &super::project_state::RenderSettings,
        audio_clips: &[(usize, f32, f32)],
        audio_store: &super::project_state::AudioStore,
    ) {
        use nezha_encoder::{Container, EncoderBackend, ExportConfig, FfmpegEncoder, QualityPreset, VideoCodec};
        use std::path::PathBuf;

        let path = match export_path {
            Some(p) => PathBuf::from(p),
            None => {
                self.state = Some(ExportState::Error("未选择导出路径".to_string()));
                return;
            }
        };

        let container: Container = match export_format.parse() {
            Ok(c) => c,
            Err(e) => {
                self.state = Some(ExportState::Error(e));
                return;
            }
        };

        let codec: VideoCodec = match encoder.parse() {
            Ok(c) => c,
            Err(e) => {
                self.state = Some(ExportState::Error(e));
                return;
            }
        };

        let backend: EncoderBackend = match encoder_backend.parse() {
            Ok(b) => b,
            Err(e) => {
                self.state = Some(ExportState::Error(format!("加速后端解析失败: {}", e)));
                return;
            }
        };

        let fps = render.fps as f64;

        let audio_pcm = if !audio_clips.is_empty() {
            Some(audio_store.mix_master(
                audio_clips,
                render.audio_sample_rate,
                duration,
            ))
        } else {
            None
        };
        let audio_channels: u16 = match render.audio_channels {
            nezha_xsynth::ChannelCount::Stereo => 2,
            nezha_xsynth::ChannelCount::Mono => 1,
        };

        let config = ExportConfig {
            width: render.width,
            height: render.height,
            fps,
            container,
            codec,
            backend,
            output_path: path,
            quality: QualityPreset::default(),
            audio_pcm,
            audio_sample_rate: render.audio_sample_rate,
            audio_channels,
        };

        let total_frames = config.total_frames(duration);

        match FfmpegEncoder::new(&config) {
            Ok(encoder) => {
                let now = Instant::now();
                self.state = Some(ExportState::Exporting {
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
                self.state = Some(ExportState::Error(
                    "未找到 ffmpeg。请将 ffmpeg 放在程序所在目录或加入 PATH 环境变量。".into(),
                ));
            }
            Err(e) => {
                self.state = Some(ExportState::Error(format!("启动编码器失败: {}", e)));
            }
        }
    }

    pub fn show_overlay(&mut self, ui: &mut egui::Ui, render_fps: u32) {
        let mut dismiss = false;
        let mut force_finish = false;
        let finalizing_stats = match &self.state {
            Some(ExportState::Finalizing { .. }) => self.state.as_ref().and_then(|s| s.stats()),
            _ => None,
        };

        if let Some(status) = &self.state {
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
                            Self::render_export_progress(ui, &stats, render_fps);
                        }
                        if ui.button("取消").clicked() {
                            dismiss = true;
                        }
                    }
                    ExportState::Finalizing { .. } => {
                        if let Some(stats) = &finalizing_stats {
                            ui.label("⏳ 正在完成编码...");
                            ui.separator();
                            Self::render_export_progress(ui, stats, render_fps);
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
                        let total_secs = *total_frames as f64 / render_fps as f64;
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
                        egui::ScrollArea::vertical()
                            .max_height(400.0)
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(msg.as_str())
                                        .color(egui::Color32::from_rgb(255, 100, 100)),
                                );
                            });
                        if ui.button("确定").clicked() {
                            dismiss = true;
                        }
                    }
                });
        }
        if force_finish {
            if let Some(stats) = finalizing_stats {
                self.state = Some(ExportState::Completed {
                    total_frames: stats.total_frames,
                    elapsed: stats.elapsed,
                    avg_fps: stats.current_fps,
                });
            }
        } else if dismiss {
            self.state = None;
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
