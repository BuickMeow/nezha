use super::App;
use crate::config_panel;
use crate::properties_panel;
use crate::sidebar;
use crate::transport;
use eframe::egui;
use nezha_media::MediaType;
use std::path::PathBuf;

use crate::app::project_state::AudioEntry;

impl App {
    pub(super) fn handle_config_action(&mut self, action: config_panel::ConfigAction) {
        match action {
            config_panel::ConfigAction::SelectMidi => self.pick_midi_file(),
            config_panel::ConfigAction::AddWaterfall => {
                self.add_waterfall_with_audio_prompt();
            }
            config_panel::ConfigAction::AddSolidColor => {
                let duration = self.project.duration() as f32;
                let color = egui::Color32::from_rgb(200, 80, 80);
                self.project
                    .timeline_state
                    .push_solid_color_clip(color, duration);
            }
            config_panel::ConfigAction::AddCounter => {
                let duration = self.project.duration() as f32;
                let midi_idx = self.project.midi.highlighted_idx;
                self.project.timeline_state.push_counter_clip(duration, midi_idx);
            }
            config_panel::ConfigAction::RemoveMidi(idx) => {
                self.project.remove_midi(idx);
                self.render_ctx.reset_midi_state();
            }
            config_panel::ConfigAction::StartExport => {
                self.start_export();
            }
            config_panel::ConfigAction::AddSoundfont => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SoundFont", &["sf2", "sfz"])
                    .pick_file()
                {
                    self.project
                        .soundfonts
                        .push(crate::app::project_state::SoundFontEntry { path });
                }
            }
            config_panel::ConfigAction::RemoveSoundfont(idx) => {
                if idx < self.project.soundfonts.len() {
                    self.project.soundfonts.remove(idx);
                }
            }
            config_panel::ConfigAction::MoveSoundfontUp(idx) => {
                if idx > 0 && idx < self.project.soundfonts.len() {
                    self.project.soundfonts.swap(idx, idx - 1);
                }
            }
            config_panel::ConfigAction::MoveSoundfontDown(idx) => {
                if idx + 1 < self.project.soundfonts.len() {
                    self.project.soundfonts.swap(idx, idx + 1);
                }
            }
            config_panel::ConfigAction::RenderAudio(midi_idx) => {
                if let Some(entry) = self.project.midi.entries.get(midi_idx) {
                    let name = PathBuf::from(&entry.path)
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("MIDI")
                        .to_string();
                    self.audio_manager.prepare_render(name, entry.path.clone());
                }
            }
            config_panel::ConfigAction::ImportMediaVideo => {
                self.import_media_by_type(Some(MediaTypeFilter::Video));
            }
            config_panel::ConfigAction::ImportMediaAudio => {
                self.import_media_by_type(Some(MediaTypeFilter::Audio));
            }
            config_panel::ConfigAction::ImportMediaImage => {
                self.import_media_by_type(Some(MediaTypeFilter::Image));
            }
            config_panel::ConfigAction::AddMediaToTimeline(media_idx) => {
                self.add_media_to_timeline(media_idx);
            }
            config_panel::ConfigAction::RemoveMedia(media_idx) => {
                self.remove_media(media_idx);
            }
        }
    }

    fn import_media_by_type(&mut self, filter: Option<MediaTypeFilter>) {
        let mut dialog = rfd::FileDialog::new();
        match filter {
            Some(MediaTypeFilter::Video) => {
                dialog = dialog.add_filter(
                    "视频文件",
                    &["mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "ts", "m4v"],
                );
            }
            Some(MediaTypeFilter::Audio) => {
                dialog = dialog.add_filter(
                    "音频文件",
                    &["mp3", "wav", "flac", "ogg", "aac", "m4a", "wma", "opus"],
                );
            }
            Some(MediaTypeFilter::Image) => {
                dialog = dialog.add_filter(
                    "图片文件",
                    &["png", "jpg", "jpeg", "bmp", "webp", "tiff", "gif"],
                );
            }
            None => {
                dialog = dialog.add_filter(
                    "所有媒体文件",
                    &[
                        "mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "ts", "m4v",
                        "mp3", "wav", "flac", "ogg", "aac", "m4a", "wma", "opus",
                        "png", "jpg", "jpeg", "bmp", "webp", "tiff", "gif",
                    ],
                );
            }
        }

        if let Some(path) = dialog.pick_file() {
            let path_str = path.to_string_lossy().to_string();
            match nezha_media::probe_media(&path_str) {
                Ok(info) => {
                    match info.media_type {
                        MediaType::Image => {
                            match nezha_media::load_image(&path_str) {
                                Ok(img) => {
                                    self.project.media.add_image(
                                        info,
                                        img.rgba,
                                        img.width,
                                        img.height,
                                    );
                                }
                                Err(e) => {
                                    self.project.last_error =
                                        Some(format!("图片加载失败: {}", e));
                                }
                            }
                        }
                        MediaType::Video => {
                            self.project.media.add_video(info);
                        }
                        MediaType::Audio => {
                            self.project.media.add_audio(info);
                        }
                    }
                }
                Err(e) => {
                    self.project.last_error = Some(format!("媒体探测失败: {}", e));
                }
            }
        }
    }

    fn add_media_to_timeline(&mut self, media_idx: usize) {
        let entry = match self.project.media.get(media_idx) {
            Some(e) => e,
            None => return,
        };
        let info = &entry.info;
        let name = std::path::Path::new(&info.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("媒体")
            .to_string();

        match info.media_type {
            MediaType::Image => {
                let duration = self.project.duration() as f32;
                self.project
                    .timeline_state
                    .push_image_clip(media_idx, name, duration);
            }
            MediaType::Video => {
                let duration = info.duration_secs as f32;
                self.project
                    .timeline_state
                    .push_video_clip(media_idx, name, duration);
            }
            MediaType::Audio => {
                let sample_rate = self.project.render.audio_sample_rate;
                if let Some(decoded) = self.project.media.decode_audio(media_idx, sample_rate) {
                    let audio_entry = AudioEntry {
                        name: name.clone(),
                        sample_rate: decoded.sample_rate,
                        channels: decoded.channels,
                        duration_secs: decoded.duration_secs,
                        samples: decoded.samples,
                        midi_idx: 0,
                    };
                    let audio_idx = self.project.audio.insert(audio_entry);
                    self.project.timeline_state.push_audio_clip(
                        audio_idx,
                        name,
                        info.duration_secs as f32,
                    );
                } else {
                    self.project.last_error = Some("音频解码失败".to_string());
                }
            }
        }
    }

    fn remove_media(&mut self, media_idx: usize) {
        if media_idx < self.project.media.len() {
            self.project.media.entries.remove(media_idx);
            self.project.media.video_decoders.remove(&media_idx);
        }
    }

    pub(super) fn render_side_panels(&mut self, ui: &mut egui::Ui) {
        let mut config_action = None;
        let dark_mode = self.ui.theme_mode.is_dark(ui.ctx());
        self.project.timeline_state.fps = self.project.render.fps;
        let duration = self.project.duration() as f32;

        egui::Panel::left("sidebar")
            .exact_size(60.0)
            .resizable(false)
            .show_inside(ui, |ui| {
                sidebar::show(
                    ui,
                    &mut self.ui.active_tab,
                    &mut self.ui.config_panel_visible,
                );
            });

        egui::Panel::bottom("transport")
            .exact_size(200.0)
            .resizable(false)
            .show_inside(ui, |ui| {
                let mut transport_time = self.project.playback.current_time as f32;
                transport::show(
                    ui,
                    &mut self.project.playback.is_playing,
                    &mut transport_time,
                    duration,
                    &mut self.project.timeline_state,
                    dark_mode,
                );
                self.project.playback.current_time = transport_time as f64;
            });

        if self.ui.config_panel_visible {
            egui::Panel::left("config_panel")
                .exact_size(260.0)
                .min_size(260.0)
                .max_size(260.0)
                .resizable(false)
                .show_inside(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    ui.set_min_width(ui.available_width());

                    let mut state = config_panel::ConfigState {
                        active_tab: self.ui.active_tab,
                        midi_files: &self.project.midi.entries,
                        highlighted_midi_idx: &mut self.project.midi.highlighted_idx,
                        render_width: &mut self.project.render.width,
                        render_height: &mut self.project.render.height,
                        fps: &mut self.project.render.fps,
                        export_format: &mut self.ui.export_format,
                        encoder: &mut self.ui.encoder,
                        encoder_backend: &mut self.ui.encoder_backend,
                        export_path: &mut self.ui.export_path,
                        theme_mode: &mut self.ui.theme_mode,
                        soundfonts: &self.project.soundfonts,
                        audio_device_name: &mut self.ui.audio_device_name,
                        audio_devices: &self.ui.audio_devices,
                        media: &mut self.project.media,
                        timeline: &mut self.project.timeline_state,
                        audio: &mut self.project.audio,
                    };

                    if let Some(action) = config_panel::show(ui, &mut state) {
                        config_action = Some(action);
                    }
                });
        }

        if self
            .project
            .timeline_state
            .selection
            .selected_clip_id
            .is_some()
        {
            egui::Panel::right("properties_panel")
                .exact_size(220.0)
                .min_size(220.0)
                .max_size(220.0)
                .resizable(false)
                .show_inside(ui, |ui| {
                    ui.set_max_width(ui.available_width());
                    ui.set_min_width(ui.available_width());

                    properties_panel::show(
                        ui,
                        &mut self.project.timeline_state,
                        self.ui.zoom,
                        &self.project.midi.entries,
                    );
                });
        }

        if let Some(action) = config_action {
            self.handle_config_action(action);
        }
    }
}

enum MediaTypeFilter {
    Video,
    Audio,
    Image,
}
