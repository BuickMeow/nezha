use std::path::PathBuf;
use std::sync::mpsc;

use crate::app::audio_player::AudioPlayback;
use crate::app::project_state::{AudioEntry, ProjectState};
use crate::transport::{TrackClip, TrackKind};
use nezha_xsynth::ChannelCount;

// ── Events from the render thread ──

enum AudioRenderEvent {
    Chunk(Vec<f32>, f64, u64, f64),
    Done,
    Error(String),
}

// ── Render thread state ──

struct RenderState {
    midi_idx: usize,
    rx: mpsc::Receiver<AudioRenderEvent>,
    accumulated_pcm: Vec<f32>,
    last_progress: f64,
    last_voice_count: u64,
}

// ── Audio manager ──

pub struct AudioManager {
    render_state: Option<RenderState>,
    pub render_settings_open: bool,
    pub render_progress_open: bool,
    pub cached_midi_name: String,
    pub cached_midi_path: String,
}

impl AudioManager {
    pub fn new() -> Self {
        Self {
            render_state: None,
            render_settings_open: false,
            render_progress_open: false,
            cached_midi_name: String::new(),
            cached_midi_path: String::new(),
        }
    }

    /// Current render progress info for the dialog.
    pub fn progress_info(&self) -> (f64, u64) {
        match self.render_state {
            Some(ref s) => (s.last_progress, s.last_voice_count),
            None => (0.0, 0),
        }
    }

    /// Prepare to show the render settings dialog for a MIDI file.
    pub fn prepare_render(&mut self, name: String, path: String) {
        self.cached_midi_name = name;
        self.cached_midi_path = path;
        self.render_settings_open = true;
    }

    /// Start the xsynth render in a background thread.
    pub fn start_render(
        &mut self,
        midi_idx: usize,
        midi_path: &str,
        sample_rate: u32,
        channels: ChannelCount,
        use_limiter: bool,
        layers: u32,
        min_velocity: u8,
        soundfont_paths: &[PathBuf],
    ) {
        let midi_data = match std::fs::read(midi_path) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("AudioManager: read MIDI failed: {}", e);
                return;
            }
        };

        let config = nezha_xsynth::RenderConfig {
            sample_rate,
            channels,
            use_limiter,
            layers: Some(layers as usize),
            min_velocity,
        };

        let sfonts = soundfont_paths.to_vec();
        let (tx, rx) = mpsc::channel();
        let tx2 = tx.clone();

        std::thread::spawn(move || {
            let result = nezha_xsynth::render_midi_to_pcm_chunked(
                &midi_data,
                &sfonts,
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
                Ok(_) => {
                    let _ = tx2.send(AudioRenderEvent::Done);
                }
                Err(e) => {
                    let _ = tx2.send(AudioRenderEvent::Error(e.to_string()));
                }
            }
        });

        self.render_state = Some(RenderState {
            midi_idx,
            rx,
            accumulated_pcm: Vec::new(),
            last_progress: 0.0,
            last_voice_count: 0,
        });
        self.render_progress_open = true;
    }

    /// Call every frame. Polls the render thread and updates project state.
    /// Returns true if the project/audio was modified (for UI refresh hints).
    pub fn poll(&mut self, project: &mut ProjectState, audio_player: &mut AudioPlayback) -> bool {
        let mut state = match self.render_state.take() {
            Some(s) => s,
            None => return false,
        };

        let mut modified = false;

        loop {
            match state.rx.try_recv() {
                Ok(AudioRenderEvent::Chunk(pcm, _elapsed, voice, total)) => {
                    state.last_voice_count = voice;
                    state.last_progress = if total > 0.0 {
                        (_elapsed / total).min(1.0)
                    } else {
                        0.0
                    };
                    state.accumulated_pcm.extend(pcm);
                    modified = true;
                }

                Ok(AudioRenderEvent::Done) => {
                    if !state.accumulated_pcm.is_empty() {
                        let ch = match project.render.audio_channels {
                            ChannelCount::Stereo => 2,
                            ChannelCount::Mono => 1,
                        } as f64;
                        let duration_secs = state.accumulated_pcm.len() as f64
                            / (project.render.audio_sample_rate as f64 * ch);

                        let entry_data = state.accumulated_pcm;
                        let existing = project
                            .audio
                            .entries
                            .iter()
                            .position(|e| e.midi_idx == state.midi_idx);

                        if let Some(idx) = existing {
                            project.audio.entries[idx] = AudioEntry {
                                name: self.cached_midi_name.clone(),
                                sample_rate: project.render.audio_sample_rate,
                                channels: match project.render.audio_channels {
                                    ChannelCount::Stereo => 2,
                                    ChannelCount::Mono => 1,
                                },
                                duration_secs,
                                samples: entry_data,
                                midi_idx: state.midi_idx,
                            };
                            for track in &mut project.timeline_state.data.tracks {
                                for clip in &mut track.clips {
                                    if clip.audio_idx == Some(idx)
                                        && clip.kind == crate::transport::ClipKind::Audio
                                    {
                                        clip.end = duration_secs as f32;
                                    }
                                }
                            }
                        } else {
                            let entry = AudioEntry {
                                name: self.cached_midi_name.clone(),
                                sample_rate: project.render.audio_sample_rate,
                                channels: match project.render.audio_channels {
                                    ChannelCount::Stereo => 2,
                                    ChannelCount::Mono => 1,
                                },
                                duration_secs,
                                samples: entry_data,
                                midi_idx: state.midi_idx,
                            };
                            let audio_idx = project.audio.insert(entry);
                            let id = project.timeline_state.data.alloc_clip_id();
                            let clip = TrackClip::new_audio(
                                id,
                                format!("音频: {}", self.cached_midi_name),
                                audio_idx,
                                duration_secs as f32,
                            );
                            if let Some(empty_track) = project
                                .timeline_state
                                .data
                                .tracks
                                .iter_mut()
                                .find(|t| t.kind == TrackKind::Audio && t.clips.is_empty())
                            {
                                empty_track.clips.push(clip);
                            } else {
                                let name = crate::transport::next_audio_track_name(
                                    &project.timeline_state.data.tracks,
                                );
                                let mut track = crate::transport::Track::new_audio(&name);
                                track.clips.push(clip);
                                project.timeline_state.data.tracks.push(track);
                            }
                        }

                        project
                            .timeline_state
                            .data
                            .update_duration(duration_secs as f32);

                        let audio_clips = project.audio_timeline_clips();
                        audio_player.mix(
                            &project.audio,
                            &audio_clips,
                            project.duration(),
                            project.render.audio_sample_rate,
                        );

                        modified = true;
                    }
                    self.render_progress_open = false;
                    return modified;
                }

                Ok(AudioRenderEvent::Error(e)) => {
                    project.last_error = Some(format!("音频渲染失败: {}", e));
                    self.render_progress_open = false;
                    return modified;
                }

                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    project.last_error = Some("音频渲染线程意外退出".to_string());
                    self.render_progress_open = false;
                    return modified;
                }
            }
        }

        self.render_state = Some(state);
        modified
    }
}
