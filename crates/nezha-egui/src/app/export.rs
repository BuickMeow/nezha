use super::App;
use eframe::egui;
use nezha_encoder::{
    Container, EncoderError, ExportConfig, FfmpegEncoder, QualityPreset, VideoCodec,
};
use std::path::PathBuf;

#[derive(Debug)]
pub enum ExportState {
    Exporting {
        encoder: FfmpegEncoder,
        current_frame: u64,
        total_frames: u64,
    },
    Completed,
    Error(String),
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
                self.export_state = Some(ExportState::Exporting {
                    encoder,
                    current_frame: 0,
                    total_frames,
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

    /// 批量导出视频帧。
    ///
    /// 每帧 UI update 会在一个时间预算内（约 50ms）尽可能多地渲染视频帧，
    /// 低负载时大幅加速，高负载时自然降速，不会阻塞 UI。
    pub(super) fn export_step(&mut self) {
        let budget = std::time::Duration::from_millis(50);
        let start = std::time::Instant::now();

        loop {
            match self.export_state.take() {
                Some(ExportState::Exporting {
                    mut encoder,
                    current_frame,
                    total_frames,
                }) => {
                    if current_frame >= total_frames {
                        // 所有帧已写入，关闭编码器
                        match encoder.finish() {
                            Ok(()) => {
                                self.export_state = Some(ExportState::Completed);
                            }
                            Err(e) => {
                                self.export_state =
                                    Some(ExportState::Error(format!("编码器收尾失败: {}", e)));
                            }
                        }
                        return;
                    }

                    let time = current_frame as f64 / self.project.render.fps as f64;
                    self.render_frame_for_export(time as f32);
                    let bytes = self.render_ctx.read_frame_bytes();
                    if bytes.is_empty() {
                        self.export_state =
                            Some(ExportState::Error("GPU 帧读取失败或超时".to_string()));
                        return;
                    }

                    match encoder.write_frame(&bytes) {
                        Ok(()) => {
                            let next_frame = current_frame + 1;
                            if next_frame >= total_frames {
                                // 本轮已写完最后一帧，下一轮循环会进入收尾逻辑
                                self.export_state = Some(ExportState::Exporting {
                                    encoder,
                                    current_frame: next_frame,
                                    total_frames,
                                });
                                // 继续循环以立即完成收尾，避免多等一帧 UI
                                continue;
                            }

                            if start.elapsed() < budget {
                                // 时间预算还有剩余，继续渲染下一帧
                                self.export_state = Some(ExportState::Exporting {
                                    encoder,
                                    current_frame: next_frame,
                                    total_frames,
                                });
                                continue;
                            }

                            // 时间预算用完，等下一帧 UI 再继续
                            self.export_state = Some(ExportState::Exporting {
                                encoder,
                                current_frame: next_frame,
                                total_frames,
                            });
                            return;
                        }
                        Err(e) => {
                            self.export_state =
                                Some(ExportState::Error(format!("写入视频帧失败: {}", e)));
                            return;
                        }
                    }
                }
                other => {
                    self.export_state = other;
                    return;
                }
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
                    ExportState::Exporting {
                        current_frame,
                        total_frames,
                        ..
                    } => {
                        let progress = *current_frame as f32 / (*total_frames).max(1) as f32;
                        ui.label(format!("正在导出... {} / {}", current_frame, total_frames));
                        ui.add(egui::ProgressBar::new(progress).show_percentage());
                        if ui.button("取消").clicked() {
                            dismiss = true;
                        }
                    }
                    ExportState::Completed => {
                        ui.label("导出完成！");
                        if ui.button("确定").clicked() {
                            dismiss = true;
                        }
                    }
                    ExportState::Error(msg) => {
                        ui.label("导出失败");
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
