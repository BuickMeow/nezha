use super::export_controller::ExportState;
use super::App;
use eframe::egui;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

impl App {
    pub(super) fn start_export(&mut self) {
        let audio_clips = self.project.audio_timeline_clips();
        self.project.playback.is_playing = false;
        self.export.start(
            self.ui.export_path.as_deref(),
            &self.ui.export_format,
            &self.ui.encoder,
            &self.ui.encoder_backend,
            self.project.duration(),
            &self.project.render,
            &audio_clips,
            &self.project.audio,
        );
    }

    const MAX_BATCH_DURATION_MS: u64 = 20;
    const STAT_UPDATE_INTERVAL: Duration = Duration::from_millis(500);
    const EMA_SMOOTHING_PREV: f64 = 0.6;
    const EMA_SMOOTHING_CURR: f64 = 0.4;

    pub(super) fn export_step(&mut self) {
        match self.export.state.take() {
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
                    // Phase 1: try to read completed frames and write to ffmpeg
                    while let Some(data) = self.renderer.export_pipeline.try_read() {
                        match encoder.write_frame(data) {
                            Ok(()) => {
                                written_frame += 1;
                                frames_since_stat += 1;
                            }
                            Err(e) => {
                                self.export.state =
                                    Some(ExportState::Error(format!("写入视频帧失败: {}", e)));
                                return;
                            }
                        }
                    }

                    // Phase 2: render new frame if ring has capacity
                    if rendered_frame < total_frames && self.renderer.export_pipeline.can_write() {
                        let time = rendered_frame as f64 / fps;
                        self.renderer
                            .render_frame_pipelined(time as f32, &mut self.project);
                        rendered_frame += 1;
                        continue;
                    }

                    // Phase 3: termination check
                    let all_rendered = rendered_frame >= total_frames;
                    let all_written = written_frame >= total_frames;

                    if all_rendered && all_written {
                        let (tx, rx) = std::sync::mpsc::channel();
                        std::thread::spawn(move || {
                            let result = encoder.finish();
                            let _ = tx.send(result);
                        });
                        self.export.state = Some(ExportState::Finalizing {
                            started_at,
                            total_frames,
                            written_frame,
                            smoothed_fps,
                            finish_rx: rx,
                        });
                        return;
                    }

                    if all_rendered && self.renderer.export_pipeline.has_pending() {
                        break;
                    }

                    // Phase 4: FPS stats
                    let now = Instant::now();
                    let since_stat = now.duration_since(last_stat_time);
                    if since_stat >= Self::STAT_UPDATE_INTERVAL {
                        let instant_fps =
                            frames_since_stat as f64 / since_stat.as_secs_f64().max(0.001);
                        if smoothed_fps == 0.0 {
                            smoothed_fps = instant_fps;
                        } else {
                            smoothed_fps = smoothed_fps * Self::EMA_SMOOTHING_PREV
                                + instant_fps * Self::EMA_SMOOTHING_CURR;
                        }
                        last_stat_time = now;
                        frames_since_stat = 0;
                    }

                    // Phase 5: time budget exhausted
                    if now >= batch_deadline {
                        break;
                    }

                    // Phase 6: ring full, yield to UI
                    break;
                }

                // Final FPS update at end of batch
                if frames_since_stat > 0 {
                    let since_stat = Instant::now().duration_since(last_stat_time);
                    if since_stat.as_secs_f64() > 0.0 {
                        let instant_fps =
                            frames_since_stat as f64 / since_stat.as_secs_f64().max(0.001);
                        if smoothed_fps == 0.0 {
                            smoothed_fps = instant_fps;
                        } else {
                            smoothed_fps = smoothed_fps * Self::EMA_SMOOTHING_PREV
                                + instant_fps * Self::EMA_SMOOTHING_CURR;
                        }
                    }
                }

                self.export.state = Some(ExportState::Exporting {
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

            // Finalizing: wait for background ffmpeg finish
            Some(ExportState::Finalizing {
                started_at,
                total_frames,
                written_frame,
                smoothed_fps,
                finish_rx,
            }) => match finish_rx.try_recv() {
                Ok(Ok(())) => {
                    let elapsed = started_at.elapsed();
                    let avg_fps = if elapsed.as_secs_f64() > 0.0 {
                        total_frames as f64 / elapsed.as_secs_f64()
                    } else {
                        0.0
                    };
                    self.export.state = Some(ExportState::Completed {
                        total_frames,
                        elapsed,
                        avg_fps,
                    });
                }
                Ok(Err(e)) => {
                    self.export.state =
                        Some(ExportState::Error(format!("编码器收尾失败: {}", e)));
                }
                Err(TryRecvError::Empty) => {
                    self.export.state = Some(ExportState::Finalizing {
                        started_at,
                        total_frames,
                        written_frame,
                        smoothed_fps,
                        finish_rx,
                    });
                }
                Err(TryRecvError::Disconnected) => {
                    self.export.state =
                        Some(ExportState::Error("编码器线程异常退出".to_string()));
                }
            },

            other => {
                self.export.state = other;
            }
        }
    }

    pub(super) fn show_export_overlay(&mut self, ui: &mut egui::Ui) {
        self.export
            .show_overlay(ui, self.project.render.fps);
    }
}
