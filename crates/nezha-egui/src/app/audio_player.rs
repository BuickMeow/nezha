use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::app::project_state::AudioStore;

/// Shared state between the audio playback thread and the main UI thread.
pub struct AudioPlayback {
    pub is_playing: Arc<AtomicBool>,
    pub current_frame: Arc<AtomicU64>,
    mixed_buffer: Arc<Vec<f32>>,
    sample_rate: u32,
    stream: Option<cpal::Stream>,
}

impl AudioPlayback {
    pub fn new() -> Self {
        Self {
            is_playing: Arc::new(AtomicBool::new(false)),
            current_frame: Arc::new(AtomicU64::new(0)),
            mixed_buffer: Arc::new(Vec::new()),
            sample_rate: 48000,
            stream: None,
        }
    }

    /// Replace mixed buffer and sample rate. Thread-safe for the callback.
    pub fn mix(
        &mut self,
        audio_store: &AudioStore,
        timeline_clips: &[(usize, f32, f32)],
        duration_secs: f64,
        sample_rate: u32,
    ) {
        let mixed = audio_store.mix_timeline(timeline_clips, sample_rate, duration_secs);
        self.mixed_buffer = Arc::new(mixed);
        self.sample_rate = sample_rate;
    }

    pub fn play(&mut self) {
        if self.is_playing.load(Ordering::Relaxed) {
            return;
        }

        let buffer = self.mixed_buffer.clone();
        let sample_rate = self.sample_rate;
        let is_playing = self.is_playing.clone();
        let current_frame = self.current_frame.clone();

        if buffer.is_empty() || sample_rate == 0 {
            eprintln!("AudioPlayback: buffer empty or sample_rate=0, cannot play");
            return;
        }

        let channels = 2u16;

        let result = cpal::default_host()
            .output_devices()
            .ok()
            .and_then(|mut devices| {
                let dev = devices.next();
                if dev.is_none() {
                    eprintln!("AudioPlayback: no output device found");
                }
                dev
            })
            .and_then(|device| {
                let supported = device.default_output_config().ok();
                let config = supported
                    .as_ref()
                    .map(|c| c.config())
                    .unwrap_or(cpal::StreamConfig {
                        channels,
                        sample_rate: cpal::SampleRate(sample_rate),
                        buffer_size: cpal::BufferSize::Default,
                    });

                let err_fn = |err: cpal::StreamError| {
                    eprintln!("AudioPlayback stream error: {:?}", err);
                };

                let stream = device
                    .build_output_stream(
                        &config,
                        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                            if !is_playing.load(Ordering::Relaxed) {
                                data.fill(0.0);
                                return;
                            }

                            let buf_len = buffer.len();
                            let frames_avail = if buf_len > 0 {
                                buf_len / channels as usize
                            } else {
                                0
                            };

                            let start_frame = current_frame.load(Ordering::Relaxed) as usize;
                            let mut out_idx = 0;
                            let mut write_frame = start_frame;

                            while out_idx < data.len() {
                                if write_frame >= frames_avail {
                                    data[out_idx..].fill(0.0);
                                    break;
                                }
                                let buf_idx = write_frame * channels as usize;
                                let samples_to_end =
                                    (frames_avail - write_frame) * channels as usize;
                                let remaining_out = data.len() - out_idx;
                                let copy = samples_to_end.min(remaining_out);

                                data[out_idx..out_idx + copy]
                                    .copy_from_slice(&buffer[buf_idx..buf_idx + copy]);
                                out_idx += copy;
                                write_frame += copy / channels as usize;
                            }

                            current_frame.store(write_frame as u64, Ordering::Relaxed);
                        },
                        err_fn,
                        None,
                    )
                    .ok();

                if stream.is_none() {
                    eprintln!("AudioPlayback: build_output_stream failed");
                }
                stream
            });

        if let Some(stream) = result {
            // CRITICAL: start the stream playback
            if let Err(e) = stream.play() {
                eprintln!("AudioPlayback: stream.play() failed: {:?}", e);
            }
            self.stream = Some(stream);
            self.is_playing.store(true, Ordering::Relaxed);
            tracing::info!("AudioPlayback: started successfully");
        }
    }

    pub fn pause(&self) {
        self.is_playing.store(false, Ordering::Relaxed);
    }

    pub fn stop(&mut self) {
        self.pause();
        self.current_frame.store(0, Ordering::Relaxed);
        if let Some(stream) = self.stream.take() {
            let _ = stream.pause();
        }
    }

    pub fn seek_to(&self, time_sec: f64) {
        if self.sample_rate > 0 {
            let frame = (time_sec * self.sample_rate as f64) as u64;
            self.current_frame.store(frame, Ordering::Relaxed);
        }
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    pub fn current_time(&self) -> f64 {
        if self.sample_rate > 0 {
            self.current_frame.load(Ordering::Relaxed) as f64 / self.sample_rate as f64
        } else {
            0.0
        }
    }
}
