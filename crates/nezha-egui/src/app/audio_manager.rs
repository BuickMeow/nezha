use std::path::PathBuf;
use std::sync::mpsc;

use crate::app::audio_player::AudioPlayback;
use crate::app::project_state::{AudioEntry, AudioStore, ProjectState};
use crate::transport::{TrackClip, TrackKind};
use nezha_xsynth::ChannelCount;

// ── Events from the render thread ──

enum AudioRenderEvent {
    Chunk(Vec<f32>, f64, u64, f64),
    Done,
    Error(String),
}

// ── Work sent to the background mixer thread ──

struct MixerWork {
    audio_store: AudioStore,
    timeline_clips: Vec<(usize, f32, f32)>,
    sample_rate: u32,
    duration_secs: f64,
}

// ── Render thread state ──

struct RenderState {
    midi_idx: usize,
    rx: mpsc::Receiver<AudioRenderEvent>,
    accumulated_pcm: Vec<f32>,
    last_progress: f64,
    last_voice_count: u64,
    /// 已创建的音频 entry 索引（None 表示尚未创建）。
    audio_idx: Option<usize>,
}

// ── Audio manager ──

pub struct AudioManager {
    render_state: Option<RenderState>,
    pub render_settings_open: bool,
    pub render_progress_open: bool,
    pub cached_midi_name: String,
    pub cached_midi_path: String,
    /// 后台混音线程通信句柄。
    mixer_tx: Option<mpsc::Sender<MixerWork>>,
    mixer_handle: Option<std::thread::JoinHandle<()>>,
}

impl AudioManager {
    pub fn new() -> Self {
        Self {
            render_state: None,
            render_settings_open: false,
            render_progress_open: false,
            cached_midi_name: String::new(),
            cached_midi_path: String::new(),
            mixer_tx: None,
            mixer_handle: None,
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
    #[allow(clippy::too_many_arguments)]
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
            ..Default::default()
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
            audio_idx: None,
        });
        self.render_progress_open = true;
    }

    /// 查找与指定 midi_idx 关联的瀑布流 Clip 的 song_start_time。
    fn find_song_start(project: &ProjectState, midi_idx: usize) -> f32 {
        project
            .timeline_state
            .data
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .find(|c| {
                c.kind == crate::transport::ClipKind::Waterfall && c.midi_idx == Some(midi_idx)
            })
            .map(|c| c.song_start_time)
            .unwrap_or(0.0)
    }

    /// 创建或更新音频 entry 和 Clip（不再在 UI 线程混音）。
    fn upsert_audio(&mut self, state: &mut RenderState, project: &mut ProjectState) {
        if state.accumulated_pcm.is_empty() {
            return;
        }

        let ch = match project.render.audio_channels {
            ChannelCount::Stereo => 2,
            ChannelCount::Mono => 1,
        } as f64;
        let duration_secs =
            state.accumulated_pcm.len() as f64 / (project.render.audio_sample_rate as f64 * ch);
        let song_start = Self::find_song_start(project, state.midi_idx);

        if let Some(idx) = state.audio_idx {
            // ── 已有 entry：更新 samples 和 Clip.end ──
            if let Some(entry) = project.audio.entries.get_mut(idx) {
                entry.samples = state.accumulated_pcm.clone();
                entry.duration_secs = duration_secs;
            }
            for track in &mut project.timeline_state.data.tracks {
                for clip in &mut track.clips {
                    if clip.audio_idx == Some(idx)
                        && clip.kind == crate::transport::ClipKind::Audio
                    {
                        clip.end = clip.start + duration_secs as f32;
                    }
                }
            }
        } else {
            // ── 首次：创建 entry + Clip ──
            let entry = AudioEntry {
                name: self.cached_midi_name.clone(),
                sample_rate: project.render.audio_sample_rate,
                channels: ch as u16,
                duration_secs,
                samples: state.accumulated_pcm.clone(),
                midi_idx: state.midi_idx,
            };
            let audio_idx = project.audio.insert(entry);
            state.audio_idx = Some(audio_idx);

            let id = project.timeline_state.data.alloc_clip_id();
            let mut clip = TrackClip::new_audio(
                id,
                format!("音频: {}", self.cached_midi_name),
                audio_idx,
                duration_secs as f32,
            );
            clip.start = song_start;
            clip.end = song_start + duration_secs as f32;

            if let Some(empty_track) = project
                .timeline_state
                .data
                .tracks
                .iter_mut()
                .find(|t| t.kind == TrackKind::Audio && t.clips.is_empty())
            {
                empty_track.clips.push(clip);
            } else {
                let name =
                    crate::transport::next_audio_track_name(&project.timeline_state.data.tracks);
                let mut track = crate::transport::Track::new_audio(&name);
                track.clips.push(clip);
                project.timeline_state.data.tracks.push(track);
            }
        }

        project
            .timeline_state
            .data
            .update_duration(song_start + duration_secs as f32);
    }

    /// 如果混音线程尚未启动，则从 AudioPlayback 拿到 mixed_buffer Arc
    /// 并启动一个后台线程专门执行 mix_master。
    fn ensure_mixer_thread(&mut self, audio_player: &AudioPlayback) {
        if self.mixer_tx.is_some() {
            return;
        }

        let mixed_buffer = audio_player.mixed_buffer_arc();
        let (tx, rx) = mpsc::channel::<MixerWork>();

        let handle = std::thread::Builder::new()
            .name("nezha_mixer".to_string())
            .spawn(move || {
                while let Ok(work) = rx.recv() {
                    // 丢弃所有积压的旧 work，只处理最新的一份。
                    // 这避免当 mix_master 慢于 chunk 到达速度时产生积压。
                    let mut latest = work;
                    while let Ok(w) = rx.try_recv() {
                        latest = w;
                    }

                    let mixed = latest.audio_store.mix_master(
                        &latest.timeline_clips,
                        latest.sample_rate,
                        latest.duration_secs,
                    );

                    if let Ok(mut buf) = mixed_buffer.lock() {
                        *buf = mixed;
                    }
                }
            })
            .expect("failed to spawn mixer thread");

        self.mixer_tx = Some(tx);
        self.mixer_handle = Some(handle);
    }

    /// 将当前 project 的快照发送给后台混音线程。
    fn trigger_mix(&self, project: &ProjectState) {
        let Some(ref tx) = self.mixer_tx else {
            return;
        };

        let work = MixerWork {
            audio_store: project.audio.clone(),
            timeline_clips: project.audio_timeline_clips(),
            sample_rate: project.render.audio_sample_rate,
            duration_secs: project.duration(),
        };

        // 允许失败（通道断开 = 线程已退出）
        let _ = tx.send(work);
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
                    self.upsert_audio(&mut state, project);
                    self.ensure_mixer_thread(audio_player);
                    self.trigger_mix(project);
                    modified = true;
                }

                Ok(AudioRenderEvent::Done) => {
                    self.upsert_audio(&mut state, project);
                    self.trigger_mix(project);
                    modified = true;
                    self.render_progress_open = false;
                    // 断开 channel，后台线程 recv 失败后自然退出。
                    self.mixer_tx = None;
                    self.mixer_handle = None;
                    return modified;
                }

                Ok(AudioRenderEvent::Error(e)) => {
                    project.last_error = Some(format!("音频渲染失败: {}", e));
                    self.render_progress_open = false;
                    self.mixer_tx = None;
                    self.mixer_handle = None;
                    return modified;
                }

                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    project.last_error = Some("音频渲染线程意外退出".to_string());
                    self.render_progress_open = false;
                    self.mixer_tx = None;
                    self.mixer_handle = None;
                    return modified;
                }
            }
        }

        self.render_state = Some(state);
        modified
    }
}
