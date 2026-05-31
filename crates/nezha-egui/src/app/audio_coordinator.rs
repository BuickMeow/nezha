use super::audio_manager::AudioManager;
use super::audio_player::AudioPlayback;
use super::project_state::ProjectState;
use super::ui_state::UiState;
use eframe::egui;

pub(crate) struct AudioCoordinator {
    pub player: AudioPlayback,
    pub manager: AudioManager,
    /// Last audio clip snapshot for change detection.
    last_clips: Vec<(usize, f32, f32)>,
}

impl AudioCoordinator {
    pub fn new() -> Self {
        Self {
            player: AudioPlayback::new(),
            manager: AudioManager::new(),
            last_clips: Vec::new(),
        }
    }

    /// Poll the audio render thread (call every frame).
    pub fn poll(&mut self, project: &mut ProjectState) {
        self.manager.poll(project, &mut self.player);
    }

    /// Synchronize audio playback with project timeline state.
    pub fn sync(&mut self, project: &mut ProjectState, ui: &UiState) {
        if project.playback.is_playing {
            let audio_clips = project.audio_timeline_clips();
            if audio_clips.is_empty() {
                if self.player.is_playing() {
                    self.player.pause();
                }
                return;
            }
            let clips_changed = audio_clips != self.last_clips;
            if clips_changed {
                self.last_clips.clone_from(&audio_clips);
            }

            let mixer_active = self.manager.is_mixer_active();

            if !self.player.is_playing() {
                self.player.set_device(ui.audio_device_name.clone());
                if !mixer_active {
                    self.player.mix(
                        &project.audio,
                        &audio_clips,
                        project.duration(),
                        project.render.audio_sample_rate,
                    );
                }
                let ct = project
                    .playback
                    .current_time
                    .clamp(0.0, project.duration());
                let start_frame = (ct * project.render.audio_sample_rate as f64) as u64;
                tracing::info!(
                    "PLAY: ct={:.3}s fr={} buf={} clips={} mixer={}",
                    ct,
                    start_frame,
                    self.player.buffer_len(),
                    audio_clips.len(),
                    mixer_active,
                );
                self.player.play(start_frame);
            } else if clips_changed && !mixer_active {
                self.player.mix(
                    &project.audio,
                    &audio_clips,
                    project.duration(),
                    project.render.audio_sample_rate,
                );
            }
        } else if self.player.is_playing() {
            tracing::info!("PAUSE at ct={:.3}s", project.playback.current_time);
            self.player.pause();
        }
    }

    /// Show the audio render settings and progress dialogs.
    pub fn show_dialog(&mut self, ui: &mut egui::Ui, project: &mut ProjectState) {
        if self.manager.render_settings_open {
            use crate::config_panel::project::audio_render_dialog;
            let mut open = true;

            let result = audio_render_dialog(
                ui.ctx(),
                &self.manager.cached_midi_name,
                &project.soundfonts,
                &mut project.render,
                &mut open,
            );

            if !open {
                self.manager.render_settings_open = false;
            }

            if let Some(crate::config_panel::project::AudioRenderAction::Start) = result {
                let midi_path = self.manager.cached_midi_path.clone();
                let midi_idx = project
                    .midi
                    .entries
                    .iter()
                    .position(|e| e.path == midi_path);
                if let Some(idx) = midi_idx {
                    if let Some(entry) = project.midi.entries.get(idx) {
                        let sf_paths: Vec<_> = project
                            .soundfonts
                            .iter()
                            .map(|sf| sf.path.clone())
                            .collect();
                        self.manager
                            .start_render(&super::audio_manager::RenderParams {
                                midi_idx: idx,
                                midi: &entry.file,
                                sample_rate: project.render.audio_sample_rate,
                                channels: project.render.audio_channels,
                                use_limiter: project.render.audio_use_limiter,
                                layers: project.render.audio_layers,
                                min_velocity: project.render.audio_min_velocity,
                                soundfont_paths: &sf_paths,
                            });
                    }
                }
            }
        }

        if self.manager.render_progress_open {
            let (progress, voice) = self.manager.progress_info();
            crate::config_panel::project::audio_progress_dialog(
                ui.ctx(),
                progress,
                voice,
                &mut self.manager.render_progress_open,
            );
        }
    }
}
