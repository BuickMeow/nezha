use eframe::egui;

mod archive_picker;
mod audio_manager;
mod audio_player;
pub(crate) mod constants;
pub(crate) mod error;
mod export;
mod loading;
mod media_ops;
mod panels;
mod playback;
mod preview;
mod preview_layer;
pub mod project_state;
mod render_context;
mod ui_state;

use audio_manager::AudioManager;
use audio_player::AudioPlayback;
use loading::MidiLoader;
pub use project_state::ProjectState;
pub use render_context::RenderContext;
pub use ui_state::{ThemeMode, UiState};

pub struct App {
    pub(crate) render_ctx: RenderContext,
    pub(crate) export_pipeline: render_context::export::ExportPipeline,
    pub project: ProjectState,
    pub ui: UiState,
    pub export_state: Option<export::ExportState>,
    midi_loader: Option<MidiLoader>,
    archive_picker: Option<archive_picker::ArchivePickerState>,
    pub(crate) font_atlas: nezha_text::FontAtlas,
    pub(crate) audio_player: AudioPlayback,
    pub(crate) audio_manager: AudioManager,
    /// 每个 Counter clip 的运行时统计状态（按 clip_id）。
    pub(crate) counter_stats: std::collections::HashMap<usize, crate::app::preview::CounterStats>,
    /// 每个视频素材的缓存 ImageLayer（按 media_idx）。
    pub(crate) video_layer_cache: std::collections::HashMap<usize, nezha_compositor::ImageLayer>,
    /// 每个图片素材的缓存 ImageLayer（按 media_idx）。
    pub(crate) image_layer_cache: std::collections::HashMap<usize, nezha_compositor::ImageLayer>,
    /// 上一帧的音频 clip 列表，用于检测静音状态变化。
    last_audio_clips: Vec<(usize, f32, f32)>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(feature = "profiling")]
        {
            puffin::set_scopes_on(true);
            let _ = std::mem::ManuallyDrop::new(
                puffin_http::Server::new(&format!(
                    "0.0.0.0:{}",
                    nezha_renderer::constants::PUFFIN_PORT
                ))
                .expect("puffin_http"),
            );
            tracing::info!("Puffin bridge on :8585");
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

        let wgpu = cc
            .wgpu_render_state
            .as_ref()
            .expect("wgpu backend required");
        let default_w = constants::DEFAULT_PREVIEW_WIDTH;
        let default_h = constants::DEFAULT_PREVIEW_HEIGHT;

        let mut app = Self {
            render_ctx: RenderContext::new(cc, default_w, default_h),
            export_pipeline: render_context::export::ExportPipeline::new(
                &wgpu.device,
                default_w,
                default_h,
            ),
            project: ProjectState::new(),
            ui: UiState::default(),
            export_state: None,
            midi_loader: None,
            archive_picker: None,
            font_atlas,
            audio_player: AudioPlayback::new(),
            audio_manager: AudioManager::new(),
            counter_stats: std::collections::HashMap::new(),
            video_layer_cache: std::collections::HashMap::new(),
            image_layer_cache: std::collections::HashMap::new(),
            last_audio_clips: Vec::new(),
        };

        let cfg = crate::config::Config::load();
        cfg.apply(&mut app.ui, &mut app.project);
        app.ui.refresh_audio_devices();
        app
    }

    pub fn on_midi_loaded(&mut self, path: String, midi: nezha_core::MidiFile) {
        self.project.insert_midi(path.clone(), midi);
        let file_name = std::path::Path::new(&path)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("MIDI")
            .to_string();
        self.audio_manager.prepare_render(file_name, path);
    }

    fn save_config(&self) {
        let cfg = crate::config::Config::from_ui(&self.ui, &self.project);
        cfg.save();
    }

    fn add_waterfall_with_audio_prompt(&mut self) {
        let midi_idx = self.project.midi.highlighted_idx;
        let pre_song = crate::app::constants::DEFAULT_PRE_SONG_BUFFER;
        let (duration, song_start, song_dur) = midi_idx
            .and_then(|idx| self.project.midi.entries.get(idx))
            .map(|e| {
                let d = e.file.duration as f32;
                (pre_song + d, pre_song, d)
            })
            .unwrap_or_else(|| (self.project.duration() as f32, 0.0, 0.0));
        self.project
            .timeline_state
            .push_waterfall_clip(midi_idx, duration, song_start, song_dur);
        if let Some(idx) = midi_idx
            && let Some(entry) = self.project.midi.entries.get(idx)
        {
            let name = std::path::Path::new(&entry.path)
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("MIDI")
                .to_string();
            self.audio_manager.prepare_render(name, entry.path.clone());
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
                                let _ = tx.send(loading::MidiLoadEvent::Complete(Box::new(Err(
                                    e.into()
                                ))));
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
                        let _ = tx.send(loading::MidiLoadEvent::Complete(Box::new(
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
                let (tx, rx) = std::sync::mpsc::channel();
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
                    Some(archive_picker::ArchivePickerState::Opening { path: path_str, rx });
            } else {
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn({
                    let path = path_str.clone();
                    move || {
                        let result = nezha_core::MidiFile::load_with_progress(&path, |progress| {
                            let _ = tx.send(loading::MidiLoadEvent::Progress(progress));
                        });
                        let _ = tx.send(loading::MidiLoadEvent::Complete(Box::new(result)));
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
        if let Some(ref err) = self.project.last_error {
            let err_msg = err.to_string();
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
                                    egui::RichText::new(&err_msg)
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

    fn show_audio_dialog(&mut self, ui: &mut egui::Ui) {
        // Audio render settings dialog
        if self.audio_manager.render_settings_open {
            use crate::config_panel::project::audio_render_dialog;
            let mut open = true;

            let result = audio_render_dialog(
                ui.ctx(),
                &self.audio_manager.cached_midi_name,
                &self.project.soundfonts,
                &mut self.project.render,
                &mut open,
            );

            if !open {
                self.audio_manager.render_settings_open = false;
            }

            if let Some(crate::config_panel::project::AudioRenderAction::Start) = result {
                let midi_path = self.audio_manager.cached_midi_path.clone();
                let midi_idx = self
                    .project
                    .midi
                    .entries
                    .iter()
                    .position(|e| e.path == midi_path);
                if let Some(idx) = midi_idx {
                    let sf_paths: Vec<_> = self
                        .project
                        .soundfonts
                        .iter()
                        .map(|sf| sf.path.clone())
                        .collect();
                    let cpath = self.audio_manager.cached_midi_path.clone();
                    self.audio_manager
                        .start_render(&audio_manager::RenderParams {
                            midi_idx: idx,
                            midi_path: &cpath,
                            sample_rate: self.project.render.audio_sample_rate,
                            channels: self.project.render.audio_channels,
                            use_limiter: self.project.render.audio_use_limiter,
                            layers: self.project.render.audio_layers,
                            min_velocity: self.project.render.audio_min_velocity,
                            soundfont_paths: &sf_paths,
                        });
                }
            }
        }

        // Audio render progress dialog
        if self.audio_manager.render_progress_open {
            let (progress, voice) = self.audio_manager.progress_info();
            crate::config_panel::project::audio_progress_dialog(
                ui.ctx(),
                progress,
                voice,
                &mut self.audio_manager.render_progress_open,
            );
        }
    }
}

impl App {
    fn sync_audio_playback(&mut self) {
        if self.project.playback.is_playing {
            let audio_clips = self.project.audio_timeline_clips();
            // 所有音频轨道被静音时，直接暂停播放器，避免 mix() 空 clips 导致卡顿
            if audio_clips.is_empty() {
                if self.audio_player.is_playing() {
                    self.audio_player.pause();
                }
                return;
            }
            let clips_changed = audio_clips != self.last_audio_clips;
            if clips_changed {
                self.last_audio_clips.clone_from(&audio_clips);
            }

            let mixer_active = self.audio_manager.is_mixer_active();

            if !self.audio_player.is_playing() {
                self.audio_player
                    .set_device(self.ui.audio_device_name.clone());
                if !mixer_active {
                    self.audio_player.mix(
                        &self.project.audio,
                        &audio_clips,
                        self.project.duration(),
                        self.project.render.audio_sample_rate,
                    );
                }
                let ct = self
                    .project
                    .playback
                    .current_time
                    .clamp(0.0, self.project.duration());
                let start_frame = (ct * self.project.render.audio_sample_rate as f64) as u64;
                tracing::info!(
                    "PLAY: ct={:.3}s fr={} buf={} clips={} mixer={}",
                    ct,
                    start_frame,
                    self.audio_player.buffer_len(),
                    audio_clips.len(),
                    mixer_active,
                );
                self.audio_player.play(start_frame);
            } else if clips_changed && !mixer_active {
                self.audio_player.mix(
                    &self.project.audio,
                    &audio_clips,
                    self.project.duration(),
                    self.project.render.audio_sample_rate,
                );
            }
        } else if self.audio_player.is_playing() {
            tracing::info!("PAUSE at ct={:.3}s", self.project.playback.current_time);
            self.audio_player.pause();
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(feature = "profiling")]
        puffin::GlobalProfiler::lock().new_frame();

        self.ui.theme_mode.apply(ui.ctx());
        self.handle_input(ui);

        // Audio: poll render thread
        self.audio_manager
            .poll(&mut self.project, &mut self.audio_player);

        self.sync_audio_playback();

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

        self.save_config();

        self.show_audio_dialog(ui);

        self.show_midi_loading(ui);
        self.show_archive_picker(ui);
        self.show_error_toast(ui);
        self.show_export_overlay(ui);
    }
}
