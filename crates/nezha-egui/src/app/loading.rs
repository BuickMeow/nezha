use super::App;
use eframe::egui;
use std::sync::mpsc;

pub(super) enum MidiLoadEvent {
    /// 解析音轨的进度。
    Progress(nezha_core::LoadProgress),
    /// 自定义状态文本（用于 DMS 解压等非音轨阶段）。
    Status(String),
    Complete(Result<nezha_core::MidiFile, nezha_core::MidiError>),
}

pub(super) struct MidiLoader {
    pub(super) path: String,
    pub(super) rx: mpsc::Receiver<MidiLoadEvent>,
    pub(super) current_progress: Option<nezha_core::LoadProgress>,
    pub(super) status_message: Option<String>,
}

impl App {
    pub(super) fn show_midi_loading(&mut self, ui: &mut egui::Ui) {
        if let Some(mut loader) = self.midi_loader.take() {
            let mut done = false;
            while let Ok(event) = loader.rx.try_recv() {
                match event {
                    MidiLoadEvent::Progress(progress) => {
                        loader.current_progress = Some(progress);
                    }
                    MidiLoadEvent::Status(msg) => {
                        loader.status_message = Some(msg);
                        loader.current_progress = None;
                    }
                    MidiLoadEvent::Complete(result) => {
                        match result {
                            Ok(midi) => {
                                let path = loader.path.clone();
                                self.project.insert_midi(path, midi);
                                self.render_ctx.reset_midi_state();
                            }
                            Err(error) => {
                                self.project.last_error = Some(format!("MIDI 加载失败: {}", error));
                            }
                        }
                        done = true;
                        break;
                    }
                }
            }

            if !done {
                self.midi_loader = Some(loader);
            }
        }

        if let Some(loader) = &self.midi_loader {
            let screen_rect = ui.ctx().content_rect();
            ui.ctx()
                .layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    "midi_loading_overlay".into(),
                ))
                .rect_filled(
                    screen_rect,
                    0.0,
                    egui::Color32::from_rgba_premultiplied(0, 0, 0, 160),
                );

            egui::Window::new("正在加载 MIDI")
                .order(egui::Order::Tooltip)
                .collapsible(false)
                .resizable(false)
                .movable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    if let Some(progress) = &loader.current_progress {
                        ui.label(format!(
                            "正在解析音轨 {} / {}",
                            progress.current_track, progress.total_tracks
                        ));
                        let ratio =
                            progress.current_track as f32 / progress.total_tracks.max(1) as f32;
                        ui.add(egui::ProgressBar::new(ratio).show_percentage());
                    } else if let Some(msg) = &loader.status_message {
                        ui.label(msg);
                        ui.add(egui::Spinner::new());
                    } else {
                        ui.label("正在读取文件...");
                        ui.add(egui::Spinner::new());
                    }
                });
        }
    }
}
