use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc;

mod archive_picker;
mod audio_player;
mod export;
mod loading;
mod panels;
mod playback;
mod preview;
mod preview_layer;
pub mod project_state;
mod render_context;
mod ui_state;

use audio_player::AudioPlayback;
use loading::MidiLoader;
pub use project_state::ProjectState;
pub use render_context::RenderContext;
pub use ui_state::{ThemeMode, UiState};

/// State for audio rendering via xsynth.
struct AudioRenderState {
    /// MIDI store index being rendered.
    midi_idx: usize,
    /// MIDI path on disk (for reading bytes).
    #[allow(dead_code)]
    midi_path: String,
    /// Progress receiver from render thread.
    rx: mpsc::Receiver<AudioRenderEvent>,
    /// Whether the render dialog is open.
    show_progress: bool,
    /// Cached progress values for display
    last_progress: f64,
    last_voice_count: u64,
    /// Accumulated PCM samples (interleaved f32) built from chunks.
    accumulated_pcm: Vec<f32>,
    /// Total duration in seconds (from MIDI).
    total_seconds: f64,
}

enum AudioRenderEvent {
    /// (accumulated_pcm_chunk, elapsed_seconds, voice_count, total_seconds)
    Chunk(Vec<f32>, f64, u64, f64),
    /// Rendering completed successfully.
    Done,
    /// Rendering failed.
    Error(String),
}

pub struct App {
    pub render_ctx: RenderContext,
    pub export_pipeline: render_context::export::ExportPipeline,
    pub project: ProjectState,
    pub ui: UiState,
    pub export_state: Option<export::ExportState>,
    midi_loader: Option<MidiLoader>,
    archive_picker: Option<archive_picker::ArchivePickerState>,
    pub font_atlas: nezha_text::FontAtlas,
    // Audio
    pub audio_player: AudioPlayback,
    // Audio render dialog
    render_dialog_open: bool,
    render_settings_open: bool,
    render_progress_open: bool,
    render_state: Option<AudioRenderState>,
    cached_midi_name: String, // temp storage for dialog
    cached_midi_path: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        #[cfg(feature = "profiling")]
        {
            puffin::set_scopes_on(true);
            // Leak the server so it lives for the entire app lifetime
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
        let default_w = nezha_renderer::constants::DEFAULT_PREVIEW_WIDTH;
        let default_h = nezha_renderer::constants::DEFAULT_PREVIEW_HEIGHT;

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
            render_dialog_open: false,
            render_settings_open: false,
            render_progress_open: false,
            render_state: None,
            cached_midi_name: String::new(),
            cached_midi_path: String::new(),
        };
        app.ui.refresh_audio_devices();
        app
    }

    /// Called when a MIDI file has been fully loaded.
    /// Inserts it into the project and shows the audio render dialog.
    pub fn on_midi_loaded(&mut self, path: String, midi: nezha_core::MidiFile) {
        let _midi_idx = self.project.insert_midi(path.clone(), midi);

        // Ask if user wants to render audio
        let file_name = PathBuf::from(&path)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("MIDI")
            .to_string();
        self.cached_midi_name = file_name;
        self.cached_midi_path = path;
        self.render_settings_open = true;
    }

    /// Start the xsynth audio render in a background thread.
    pub fn start_audio_render(&mut self, midi_idx: usize, midi_path: &str) {
        let settings = &self.project.render;
        let soundfont_paths: Vec<PathBuf> = self
            .project
            .soundfonts
            .iter()
            .map(|sf| sf.path.clone())
            .collect();

        let midi_data = match std::fs::read(midi_path) {
            Ok(d) => d,
            Err(e) => {
                self.project.last_error = Some(format!("读取 MIDI 文件失败: {}", e));
                return;
            }
        };

        let config = nezha_xsynth::RenderConfig {
            sample_rate: settings.audio_sample_rate,
            channels: settings.audio_channels,
            use_limiter: settings.audio_use_limiter,
            layers: Some(settings.audio_layers as usize),
            min_velocity: settings.audio_min_velocity,
        };

        let (tx, rx) = mpsc::channel();
        let tx2 = tx.clone();

        // Spawn render thread — use chunked API for incremental preview
        std::thread::spawn(move || {
            let midi_copy = midi_data.clone();
            let result = nezha_xsynth::render_midi_to_pcm_chunked(
                &midi_copy,
                &soundfont_paths,
                &config,
                |chunk, progress| {
                    let _ = tx.send(AudioRenderEvent::Chunk(
                        chunk,
                        progress.elapsed_seconds,
                        progress.voice_count,
                        progress.total_seconds,
                    ));
                },
            );

            match result {
                Ok(()) => {
                    let _ = tx2.send(AudioRenderEvent::Done);
                }
                Err(e) => {
                    let _ = tx2.send(AudioRenderEvent::Error(e.to_string()));
                }
            }
        });

        self.render_state = Some(AudioRenderState {
            midi_idx,
            midi_path: midi_path.to_string(),
            rx,
            show_progress: true,
            last_progress: 0.0,
            last_voice_count: 0,
            accumulated_pcm: Vec::new(),
            total_seconds: 0.0,
        });
        self.render_progress_open = true;
    }

    /// Finalize a completed audio render: store PCM and create audio track.
    #[allow(dead_code)]
    fn finish_audio_render(&mut self, pcm: Vec<f32>, midi_idx: usize) {
        let entry = project_state::AudioEntry {
            name: self.cached_midi_name.clone(),
            sample_rate: self.project.render.audio_sample_rate,
            channels: match self.project.render.audio_channels {
                nezha_xsynth::ChannelCount::Stereo => 2,
                nezha_xsynth::ChannelCount::Mono => 1,
            },
            duration_secs: if self.project.render.audio_sample_rate > 0 {
                let ch = match self.project.render.audio_channels {
                    nezha_xsynth::ChannelCount::Stereo => 2,
                    nezha_xsynth::ChannelCount::Mono => 1,
                };
                pcm.len() as f64 / (self.project.render.audio_sample_rate as f64 * ch as f64)
            } else {
                0.0
            },
            samples: pcm,
            midi_idx,
        };

        let audio_idx = self.project.audio.insert(entry);

        // Create audio track clip on timeline
        let entry = self.project.audio.get(audio_idx).unwrap();
        let duration = entry.duration_secs as f32;
        let id = self.project.timeline_state.data.alloc_clip_id();
        let clip = crate::transport::TrackClip::new_audio(
            id,
            format!("音频: {}", self.cached_midi_name),
            audio_idx,
            duration,
        );

        // Add to an existing audio track or create a new one
        if let Some(empty_track) = self
            .project
            .timeline_state
            .data
            .tracks
            .iter_mut()
            .find(|t| t.kind == crate::transport::TrackKind::Audio && t.clips.is_empty())
        {
            empty_track.clips.push(clip);
        } else {
            let name =
                crate::transport::next_audio_track_name(&self.project.timeline_state.data.tracks);
            let mut track = crate::transport::Track::new_audio(&name);
            track.clips.push(clip);
            self.project.timeline_state.data.tracks.push(track);
        }

        // Update project duration
        self.project.timeline_state.data.update_duration(duration);

        // Re-mix audio for preview
        let audio_clips = self.project.audio_timeline_clips();
        self.audio_player.mix(
            &self.project.audio,
            &audio_clips,
            self.project.duration(),
            self.project.render.audio_sample_rate as u32,
        );

        // Sync playback with audio
        if self.project.playback.is_playing {
            self.audio_player
                .seek_to(self.project.playback.current_time);
        }
    }

    /// Poll the audio render thread for progress updates and PCM chunks.
    fn poll_audio_render(&mut self) {
        let mut state = match self.render_state.take() {
            Some(s) => s,
            None => return,
        };

        loop {
            match state.rx.try_recv() {
                Ok(AudioRenderEvent::Chunk(pcm, elapsed, voice, total)) => {
                    state.last_progress = if total > 0.0 {
                        (elapsed / total).min(1.0)
                    } else {
                        0.0
                    };
                    state.last_voice_count = voice;
                    state.total_seconds = total;

                    // Append chunk to accumulated PCM
                    state.accumulated_pcm.extend(pcm);

                    // Recalculate duration from accumulated PCM
                    let ch = match self.project.render.audio_channels {
                        nezha_xsynth::ChannelCount::Stereo => 2,
                        nezha_xsynth::ChannelCount::Mono => 1,
                    } as f64;
                    let duration_secs = if self.project.render.audio_sample_rate > 0 && ch > 0.0 {
                        state.accumulated_pcm.len() as f64
                            / (self.project.render.audio_sample_rate as f64 * ch)
                    } else {
                        0.0
                    };

                    // Upsert AudioEntry
                    let entry = project_state::AudioEntry {
                        name: self.cached_midi_name.clone(),
                        sample_rate: self.project.render.audio_sample_rate,
                        channels: match self.project.render.audio_channels {
                            nezha_xsynth::ChannelCount::Stereo => 2,
                            nezha_xsynth::ChannelCount::Mono => 1,
                        },
                        duration_secs,
                        samples: state.accumulated_pcm.clone(),
                        midi_idx: state.midi_idx,
                    };

                    let existing = self
                        .project
                        .audio
                        .entries
                        .iter()
                        .position(|e| e.midi_idx == state.midi_idx);

                    if let Some(idx) = existing {
                        // Update existing entry AND its clip end time
                        self.project.audio.entries[idx] = entry;
                        for track in &mut self.project.timeline_state.data.tracks {
                            for clip in &mut track.clips {
                                if clip.audio_idx == Some(idx)
                                    && clip.kind == crate::transport::ClipKind::Audio
                                {
                                    clip.end = duration_secs as f32;
                                }
                            }
                        }
                    } else {
                        // First chunk: insert entry + create audio track clip
                        let audio_idx = self.project.audio.insert(entry);
                        let id = self.project.timeline_state.data.alloc_clip_id();
                        let clip = crate::transport::TrackClip::new_audio(
                            id,
                            format!("音频: {}", self.cached_midi_name),
                            audio_idx,
                            duration_secs as f32,
                        );
                        if let Some(empty_track) = self
                            .project
                            .timeline_state
                            .data
                            .tracks
                            .iter_mut()
                            .find(|t| {
                                t.kind == crate::transport::TrackKind::Audio && t.clips.is_empty()
                            })
                        {
                            empty_track.clips.push(clip);
                        } else {
                            let name = crate::transport::next_audio_track_name(
                                &self.project.timeline_state.data.tracks,
                            );
                            let mut track = crate::transport::Track::new_audio(&name);
                            track.clips.push(clip);
                            self.project.timeline_state.data.tracks.push(track);
                        }
                    }

                    // Update project timeline duration
                    self.project
                        .timeline_state
                        .data
                        .update_duration(duration_secs as f32);

                    // Re-mix audio for preview
                    let audio_clips = self.project.audio_timeline_clips();
                    self.audio_player.mix(
                        &self.project.audio,
                        &audio_clips,
                        self.project.duration(),
                        self.project.render.audio_sample_rate as u32,
                    );
                }
                Ok(AudioRenderEvent::Done) => {
                    // Final update with complete PCM
                    if !state.accumulated_pcm.is_empty() {
                        let ch = match self.project.render.audio_channels {
                            nezha_xsynth::ChannelCount::Stereo => 2,
                            nezha_xsynth::ChannelCount::Mono => 1,
                        } as f64;
                        let duration_secs = state.accumulated_pcm.len() as f64
                            / (self.project.render.audio_sample_rate as f64 * ch);

                        let final_entry = project_state::AudioEntry {
                            name: self.cached_midi_name.clone(),
                            sample_rate: self.project.render.audio_sample_rate,
                            channels: match self.project.render.audio_channels {
                                nezha_xsynth::ChannelCount::Stereo => 2,
                                nezha_xsynth::ChannelCount::Mono => 1,
                            },
                            duration_secs,
                            samples: state.accumulated_pcm,
                            midi_idx: state.midi_idx,
                        };

                        let existing = self
                            .project
                            .audio
                            .entries
                            .iter()
                            .position(|e| e.midi_idx == state.midi_idx);
                        if let Some(idx) = existing {
                            self.project.audio.entries[idx] = final_entry;
                            // Update clip end time unconditionally
                            for track in &mut self.project.timeline_state.data.tracks {
                                for clip in &mut track.clips {
                                    if clip.audio_idx == Some(idx)
                                        && clip.kind == crate::transport::ClipKind::Audio
                                    {
                                        clip.end = duration_secs as f32;
                                    }
                                }
                            }
                            self.project
                                .timeline_state
                                .data
                                .update_duration(duration_secs as f32);
                        }
                    }
                    self.render_progress_open = false;
                    return;
                }
                Ok(AudioRenderEvent::Error(e)) => {
                    self.project.last_error = Some(format!("音频渲染失败: {}", e));
                    self.render_progress_open = false;
                    return;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.project.last_error = Some("音频渲染线程意外退出".to_string());
                    self.render_progress_open = false;
                    return;
                }
            }
        }

        if state.show_progress {
            self.render_state = Some(state);
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

    /// Add a waterfall clip and optionally ask to render audio.
    fn add_waterfall_with_audio_prompt(&mut self) {
        let midi_idx = self.project.midi.highlighted_idx;
        let duration = midi_idx
            .and_then(|idx| self.project.midi.entries.get(idx))
            .map(|e| e.file.duration as f32)
            .unwrap_or_else(|| self.project.duration() as f32);

        self.project
            .timeline_state
            .push_waterfall_clip(midi_idx, duration);

        // Ask about audio render if we have a MIDI selected
        if let Some(idx) = midi_idx {
            if let Some(entry) = self.project.midi.entries.get(idx) {
                let name = PathBuf::from(&entry.path)
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("MIDI")
                    .to_string();
                self.cached_midi_name = name;
                self.cached_midi_path = entry.path.clone();
                self.render_settings_open = true;
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
        self.poll_audio_render();

        // Sync audio playback with UI playback state
        if self.project.playback.is_playing {
            let audio_clips = self.project.audio_timeline_clips();

            if !self.audio_player.is_playing() {
                // Always start from the beginning
                self.project.playback.current_time = 0.0;
                self.audio_player.mix(
                    &self.project.audio,
                    &audio_clips,
                    self.project.duration(),
                    self.project.render.audio_sample_rate as u32,
                );
                // Always start playback from the beginning (ct=0)
                let ct = 0.0_f64;
                let start_frame = (ct * self.project.render.audio_sample_rate as f64) as u64;
                tracing::info!(
                    "PLAY: start_fr={} buf={} clips={} dur={:.3}s",
                    start_frame,
                    self.audio_player.buffer_len(),
                    audio_clips.len(),
                    self.project.duration(),
                );
                self.audio_player
                    .play(self.ui.audio_device_name.as_deref(), start_frame);
            }
        } else {
            if self.audio_player.is_playing() {
                tracing::info!(
                    "PAUSE at ct={:.3}s dur={:.3}s",
                    self.project.playback.current_time,
                    self.project.duration()
                );
                self.audio_player.pause();
            }
        }

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

        // Audio render settings dialog
        if self.render_settings_open {
            use crate::config_panel::project::audio_render_dialog;
            let mut open = true;
            let mut sample_rate = self.project.render.audio_sample_rate;
            let mut use_stereo = matches!(
                self.project.render.audio_channels,
                nezha_xsynth::ChannelCount::Stereo
            );
            let mut use_limiter = self.project.render.audio_use_limiter;
            let mut layers = self.project.render.audio_layers;
            let mut min_velocity = self.project.render.audio_min_velocity;

            let result = audio_render_dialog(
                ui.ctx(),
                &self.cached_midi_name,
                &self.project.soundfonts,
                &mut sample_rate,
                &mut use_stereo,
                &mut use_limiter,
                &mut layers,
                &mut min_velocity,
                &mut open,
            );

            // Save settings
            self.project.render.audio_sample_rate = sample_rate;
            self.project.render.audio_channels = if use_stereo {
                nezha_xsynth::ChannelCount::Stereo
            } else {
                nezha_xsynth::ChannelCount::Mono
            };
            self.project.render.audio_use_limiter = use_limiter;
            self.project.render.audio_layers = layers;
            self.project.render.audio_min_velocity = min_velocity;

            if !open {
                self.render_settings_open = false;
            }

            if let Some(crate::config_panel::project::AudioRenderAction::Start) = result {
                // Find the midi_idx from cached path
                let midi_path = self.cached_midi_path.clone();
                let midi_path2 = midi_path.clone();
                let midi_idx = self
                    .project
                    .midi
                    .entries
                    .iter()
                    .position(|e| e.path == midi_path);
                if let Some(idx) = midi_idx {
                    self.start_audio_render(idx, &midi_path2);
                }
            }
        }

        // Audio render progress dialog
        if self.render_progress_open {
            let (render_progress, voice_count) = if let Some(ref s) = self.render_state {
                (s.last_progress, s.last_voice_count)
            } else {
                (0.0, 0)
            };

            crate::config_panel::project::audio_progress_dialog(
                ui.ctx(),
                render_progress,
                voice_count,
                &mut self.render_progress_open,
            );

            if !self.render_progress_open {
                self.render_state = None;
            }
        }

        self.show_midi_loading(ui);
        self.show_archive_picker(ui);
        self.show_error_toast(ui);
        self.show_export_overlay(ui);
    }
}
