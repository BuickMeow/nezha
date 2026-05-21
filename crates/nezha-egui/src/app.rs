use eframe::egui;
mod archive_picker;
mod export;
mod loading;
mod panels;
mod playback;
mod preview;
pub mod project_state;
mod render_context;
mod ui_state;

use loading::MidiLoader;
pub use project_state::ProjectState;
pub use render_context::RenderContext;
pub use ui_state::{ThemeMode, UiState};

pub struct App {
    pub render_ctx: RenderContext,
    pub project: ProjectState,
    pub ui: UiState,
    pub export_state: Option<export::ExportState>,
    midi_loader: Option<MidiLoader>,
    archive_picker: Option<archive_picker::ArchivePickerState>,
    pub font_atlas: nezha_text::FontAtlas,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(feature = "profiling")]
        {
            puffin::set_scopes_on(true);
            // Leak the server so it lives for the entire app lifetime
            let _ = std::mem::ManuallyDrop::new(
                puffin_http::Server::new("0.0.0.0:8585").expect("puffin_http"),
            );
            println!("🔥 Puffin bridge on :8585 → puffin_viewer --url 127.0.0.1:8585");
        }

        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "MiSans".to_owned(),
            egui::FontData::from_static(include_bytes!("../../../assets/MiSans-Regular.otf"))
                .into(),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "MiSans".to_owned());
        cc.egui_ctx.set_fonts(fonts);

        let theme_mode = ThemeMode::System;
        theme_mode.apply(&cc.egui_ctx);

        let wgpu_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("wgpu backend required");
        let font =
            nezha_text::FontRef::from_bytes(include_bytes!("../../../assets/MiSans-Regular.otf"))
                .expect("failed to load MiSans font");
        let font_atlas = nezha_text::FontAtlas::new(&wgpu_state.device, &wgpu_state.queue, font);

        Self {
            render_ctx: RenderContext::new(cc, 1920, 1080),
            project: ProjectState::new(),
            ui: UiState::default(),
            export_state: None,
            midi_loader: None,
            archive_picker: None,
            font_atlas,
        }
    }

    pub fn pick_midi_file(&mut self) {
        if self.midi_loader.is_some() || self.archive_picker.is_some() {
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
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn({
                    let path = path_str.clone();
                    move || {
                        let data = match std::fs::read(&path) {
                            Ok(d) => d,
                            Err(e) => {
                                let _ = tx.send(loading::MidiLoadEvent::Complete(Err(e.into())));
                                return;
                            }
                        };
                        let result = nezha_dms::DmsFile::from_bytes_with_progress(&data, |p| {
                            let ev = match p {
                                nezha_dms::DmsLoadProgress::Decompressing => {
                                    loading::MidiLoadEvent::Status("正在解压 DMS...".into())
                                }
                                nezha_dms::DmsLoadProgress::ParsingTree => {
                                    loading::MidiLoadEvent::Status("正在解析 DMS 结构...".into())
                                }
                                nezha_dms::DmsLoadProgress::ExtractingEvents {
                                    current_track,
                                    total_tracks,
                                } => loading::MidiLoadEvent::Progress(nezha_core::LoadProgress {
                                    current_track,
                                    total_tracks,
                                }),
                                nezha_dms::DmsLoadProgress::GeneratingSmf => {
                                    loading::MidiLoadEvent::Status("正在生成 SMF...".into())
                                }
                            };
                            let _ = tx.send(ev);
                        });
                        let _ = tx.send(loading::MidiLoadEvent::Complete(result.map_err(|e| {
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("DMS 解析失败: {e}"),
                            )
                            .into()
                        })));
                    }
                });

                self.midi_loader = Some(MidiLoader {
                    path: path_str,
                    rx,
                    current_progress: None,
                    status_message: Some("正在读取 DMS 文件...".into()),
                });
            } else if archive_picker::is_archive_file(&path_str) {
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn({
                    let path = path_str.clone();
                    move || {
                        let result = nezha_archive::Archive::open(&path).map(|archive| {
                            let entries = archive.list_midi_files();
                            (archive, entries)
                        });
                        let _ = tx.send(result.map_err(|e| e.to_string()));
                    }
                });

                self.archive_picker =
                    Some(archive_picker::ArchivePickerState::Opening { path: path_str, rx });
            } else {
                let (tx, rx) = std::sync::mpsc::channel();

                std::thread::spawn({
                    let path = path_str.clone();
                    move || {
                        let result = nezha_core::MidiFile::load_with_progress(&path, |progress| {
                            let _ = tx.send(loading::MidiLoadEvent::Progress(progress));
                        });
                        let _ = tx.send(loading::MidiLoadEvent::Complete(result));
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

    fn show_error_toast(&mut self, ui: &mut egui::Ui) {
        if let Some(err) = self.project.last_error.clone() {
            let mut dismissed = false;
            let screen_rect = ui.ctx().content_rect();
            egui::Area::new("error_toast".into())
                .fixed_pos(egui::pos2(screen_rect.center().x, 32.0))
                .anchor(egui::Align2::CENTER_TOP, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .fill(egui::Color32::from_rgb(60, 30, 30))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&err)
                                        .color(egui::Color32::from_rgb(255, 180, 100)),
                                );
                                if ui.button("✕").clicked() {
                                    dismissed = true;
                                }
                            });
                        });
                });
            if dismissed {
                self.project.last_error = None;
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(feature = "profiling")]
        puffin::GlobalProfiler::lock().new_frame();

        self.ui.theme_mode.apply(ui.ctx());
        self.handle_input(ui);

        // 如果正在导出，每帧推进一帧视频渲染
        if self.export_state.is_some() {
            self.export_step();
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.render_side_panels(ui);

            egui::CentralPanel::default().show_inside(ui, |ui| {
                self.render_preview(ui);
            });

            ui.ctx().request_repaint();
        });

        self.show_midi_loading(ui);
        self.show_archive_picker(ui);
        self.show_error_toast(ui);
        self.show_export_overlay(ui);
    }
}
