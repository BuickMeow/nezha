use super::archive_picker::{self, ArchivePickerState};
use super::error::AppError;
use super::loading::{MidiLoadEvent, MidiLoader};
use super::project_state::ProjectState;
use eframe::egui;
use std::sync::mpsc;

pub(crate) enum MidiLoadResult {
    Loaded {
        path: String,
        midi: nezha_core::MidiFile,
    },
    NotReady,
}

pub(crate) struct FileLoader {
    pub midi_loader: Option<MidiLoader>,
    pub archive_picker: Option<ArchivePickerState>,
}

impl FileLoader {
    pub fn new() -> Self {
        Self {
            midi_loader: None,
            archive_picker: None,
        }
    }

    pub fn is_loading(&self) -> bool {
        self.midi_loader.is_some() || self.archive_picker.is_some()
    }

    pub fn pick_midi_file(&mut self) {
        if self.is_loading() {
            return;
        }

        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "MIDI / 压缩包 / DMS",
                &[
                    "mid", "midi", "zip", "7z", "tar", "tar.gz", "tgz", "tar.xz", "txz", "dms",
                ],
            )
            .pick_file()
        {
            let path_str = path.to_string_lossy().to_string();

            if path_str.to_lowercase().ends_with(".dms") {
                let (tx, rx) = mpsc::channel();
                std::thread::spawn({
                    let path = path_str.clone();
                    move || {
                        let data = match std::fs::read(&path) {
                            Ok(d) => d,
                            Err(e) => {
                                let _ = tx.send(MidiLoadEvent::Complete(Box::new(Err(
                                    e.into()
                                ))));
                                return;
                            }
                        };
                        let result = nezha_dms::DmsFile::from_bytes_with_progress(&data, |p| {
                            let ev = match p {
                                nezha_dms::DmsLoadProgress::Decompressing => {
                                    MidiLoadEvent::Status("正在解压 DMS...".into())
                                }
                                nezha_dms::DmsLoadProgress::ParsingTree => {
                                    MidiLoadEvent::Status("正在解析 DMS 结构...".into())
                                }
                                nezha_dms::DmsLoadProgress::ExtractingEvents {
                                    current_track,
                                    total_tracks,
                                } => MidiLoadEvent::Progress(nezha_core::LoadProgress {
                                    current_track,
                                    total_tracks,
                                }),
                                nezha_dms::DmsLoadProgress::GeneratingSmf => {
                                    MidiLoadEvent::Status("正在生成 SMF...".into())
                                }
                            };
                            let _ = tx.send(ev);
                        });
                        let _ = tx.send(MidiLoadEvent::Complete(Box::new(
                            result.map_err(|e| {
                                std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    format!("DMS 解析失败: {e}"),
                                )
                                .into()
                            }),
                        )));
                    }
                });

                self.midi_loader = Some(MidiLoader {
                    path: path_str,
                    rx,
                    current_progress: None,
                    status_message: Some("正在读取 DMS 文件...".into()),
                });
            } else if archive_picker::is_archive_file(&path_str) {
                let (tx, rx) = mpsc::channel();
                std::thread::spawn({
                    let path = path_str.clone();
                    move || {
                        let result = nezha_archive::Archive::open(&path).map(|archive| {
                            let entries = archive.list_midi_files();
                            (archive, entries)
                        });
                        let _ = tx.send(result);
                    }
                });

                self.archive_picker =
                    Some(ArchivePickerState::Opening { path: path_str, rx });
            } else {
                let (tx, rx) = mpsc::channel();
                std::thread::spawn({
                    let path = path_str.clone();
                    move || {
                        let result = nezha_core::MidiFile::load_with_progress(&path, |progress| {
                            let _ = tx.send(MidiLoadEvent::Progress(progress));
                        });
                        let _ = tx.send(MidiLoadEvent::Complete(Box::new(result)));
                    }
                });

                self.midi_loader = Some(MidiLoader {
                    path: path_str,
                    rx,
                    current_progress: None,
                    status_message: None,
                });
            }
        }
    }

    /// Poll the MIDI loading thread. Returns `Loaded` when done.
    pub fn poll_midi_loading(&mut self, project: &mut ProjectState) -> MidiLoadResult {
        if let Some(mut loader) = self.midi_loader.take() {
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
                        match *result {
                            Ok(midi) => {
                                let path = loader.path.clone();
                                return MidiLoadResult::Loaded { path, midi };
                            }
                            Err(error) => {
                                project.last_error = Some(AppError::midi_load(error));
                            }
                        }
                        return MidiLoadResult::NotReady;
                    }
                }
            }
            self.midi_loader = Some(loader);
        }
        MidiLoadResult::NotReady
    }

    /// Show the MIDI loading progress overlay.
    pub fn show_midi_loading_overlay(&self, ui: &mut egui::Ui) {
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
