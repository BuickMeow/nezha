use std::path::PathBuf;
use std::sync::mpsc;

use crate::app::audio_player::AudioPlayback;
use crate::app::error::AppError;
use crate::app::project_state::{AudioEntry, ProjectState};
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
    /// 新增 PCM samples（本 chunk 的增量）
    new_pcm: Vec<f32>,
    /// 新 PCM 在 timeline 上的起始秒数
    start_time: f64,
    sample_rate: u32,
    total_duration: f64,
    channels: u16,
    /// true = 渲染完成，执行最终 limiter pass
    is_final: bool,
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

/// Parameters for [`AudioManager::start_render`].
pub struct RenderParams<'a> {
    pub midi_idx: usize,
    pub midi: &'a nezha_core::MidiFile,
    pub sample_rate: u32,
    pub channels: ChannelCount,
    pub use_limiter: bool,
    pub layers: u32,
    pub min_velocity: u8,
    pub soundfont_paths: &'a [PathBuf],
}

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
    pub fn start_render(&mut self, p: &RenderParams<'_>) {
        let midi = p.midi.clone();

        let config = nezha_xsynth::RenderConfig {
            sample_rate: p.sample_rate,
            channels: p.channels,
            use_limiter: p.use_limiter,
            layers: Some(p.layers as usize),
            min_velocity: p.min_velocity,
            ..Default::default()
        };

        let sfonts = p.soundfont_paths.to_vec();
        let (tx, rx) = mpsc::channel();
        let tx2 = tx.clone();

        std::thread::spawn(move || {
            let result = nezha_xsynth::render_midi_to_pcm_chunked(
                &midi,
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
            midi_idx: p.midi_idx,
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

    /// 创建或更新音频 entry 和 Clip。
    /// 渲染期间不存储 PCM（由 mixer 线程维护 playback buffer），
    /// 仅在 is_final=true 时将最终 PCM 写入 AudioEntry。
    fn upsert_audio(
        &mut self,
        state: &mut RenderState,
        project: &mut ProjectState,
        is_final: bool,
    ) {
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
                if is_final {
                    entry.samples = std::mem::take(&mut state.accumulated_pcm);
                }
                entry.duration_secs = duration_secs;
            }
            for track in &mut project.timeline_state.data.tracks {
                for clip in &mut track.clips {
                    if clip.audio_idx == Some(idx) && clip.kind == crate::transport::ClipKind::Audio
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
                samples: if is_final {
                    std::mem::take(&mut state.accumulated_pcm)
                } else {
                    Vec::new()
                },
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
                let mut track = crate::transport::Track::new(&name, crate::transport::TrackKind::Audio);
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
    /// 并启动一个后台线程专门执行增量混音。
    fn ensure_mixer_thread(&mut self, audio_player: &AudioPlayback) {
        if self.mixer_tx.is_some() {
            return;
        }

        let mixed_buffer = audio_player.mixed_buffer_arc();
        let (tx, rx) = mpsc::channel::<MixerWork>();

        let handle = std::thread::Builder::new()
            .name("nezha_mixer".to_string())
            .spawn(move || {
                // 持久化混音缓冲区，渲染期间只增长不重置
                let mut raw_mix_buf: Vec<f32> = Vec::new();

                while let Ok(work) = rx.recv() {
                    // 丢弃所有积压的旧 work，只处理最新的一份
                    let mut latest = work;
                    while let Ok(w) = rx.try_recv() {
                        latest = w;
                    }

                    let total_frames =
                        (latest.total_duration * latest.sample_rate as f64).ceil() as usize;
                    let target_len = total_frames * 2; // stereo

                    // 扩展 raw_mix_buf（仅增长，零填充新区域）
                    if raw_mix_buf.len() < target_len {
                        raw_mix_buf.resize(target_len, 0.0);
                    }

                    // 将 new_pcm 混入 raw_mix_buf 的 start_time 对应位置
                    let start_frame =
                        (latest.start_time * latest.sample_rate as f64) as usize;
                    let ch = latest.channels as usize;
                    for i in 0..latest.new_pcm.len() {
                        let buf_idx = start_frame * 2 + (i / ch) * 2 + (i % ch).min(1);
                        raw_mix_buf[buf_idx] += latest.new_pcm[i];
                    }

                    // 拷贝到 playback buffer（无 limiter，快速）
                    if let Ok(mut buf) = mixed_buffer.lock() {
                        buf.clear();
                        buf.extend_from_slice(&raw_mix_buf[..target_len]);
                    }

                    // 渲染完成：执行最终 limiter pass
                    if latest.is_final {
                        let mut limiter = nezha_xsynth::Limiter::new(
                            latest.sample_rate as f32,
                            2,
                            -1.0,
                            -0.3,
                            2.0,
                            0.5,
                            100.0,
                        );
                        limiter.process(&mut raw_mix_buf[..target_len]);
                        if let Ok(mut buf) = mixed_buffer.lock() {
                            buf.clear();
                            buf.extend_from_slice(&raw_mix_buf[..target_len]);
                        }
                        raw_mix_buf.clear();
                    }
                }
            })
            .expect("failed to spawn mixer thread");

        self.mixer_tx = Some(tx);
        self.mixer_handle = Some(handle);
    }

    /// 将增量 PCM 发送给后台混音线程。
    fn trigger_mix(
        &self,
        new_pcm: Vec<f32>,
        start_time: f64,
        project: &ProjectState,
        is_final: bool,
    ) {
        let Some(ref tx) = self.mixer_tx else {
            return;
        };

        let ch = match project.render.audio_channels {
            ChannelCount::Stereo => 2,
            ChannelCount::Mono => 1,
        };

        let work = MixerWork {
            new_pcm,
            start_time,
            sample_rate: project.render.audio_sample_rate,
            total_duration: project.duration(),
            channels: ch,
            is_final,
        };

        // 非阻塞发送，丢弃失败（mixer 线程已有丢弃旧 work 逻辑）
        let _ = tx.send(work); // channel 断开 = mixer 线程已退出
    }

    /// 返回 true 表示后台混音线程正在运行（渲染中）。
    pub fn is_mixer_active(&self) -> bool {
        self.mixer_tx.is_some()
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
                    // 提取本次 chunk 的增量 PCM（新到达的部分）
                    let prev_len = state.accumulated_pcm.len();
                    state.accumulated_pcm.extend(pcm);
                    let new_pcm = state.accumulated_pcm[prev_len..].to_vec();
                    let start_time = prev_len as f64
                        / (project.render.audio_sample_rate as f64
                            * match project.render.audio_channels {
                                ChannelCount::Stereo => 2.0,
                                ChannelCount::Mono => 1.0,
                            });

                    self.upsert_audio(&mut state, project, false);
                    self.ensure_mixer_thread(audio_player);
                    self.trigger_mix(new_pcm, start_time, project, false);
                    modified = true;
                }

                Ok(AudioRenderEvent::Done) => {
                    self.upsert_audio(&mut state, project, true);
                    self.ensure_mixer_thread(audio_player);
                    // 发送最终 PCM（全部）以触发 limiter
                    let final_pcm = std::mem::take(&mut state.accumulated_pcm);
                    self.trigger_mix(final_pcm, 0.0, project, true);
                    modified = true;
                    self.render_progress_open = false;
                    // 断开 channel，后台线程 recv 失败后自然退出
                    self.mixer_tx = None;
                    self.mixer_handle = None;
                    return modified;
                }

                Ok(AudioRenderEvent::Error(e)) => {
                    project.last_error = Some(AppError::audio_render(e));
                    self.render_progress_open = false;
                    self.mixer_tx = None;
                    self.mixer_handle = None;
                    return modified;
                }

                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    project.last_error = Some(AppError::Other("音频渲染线程意外退出".into()));
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
