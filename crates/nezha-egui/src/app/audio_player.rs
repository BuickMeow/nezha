use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::app::project_state::AudioStore;

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

    pub fn play(&mut self, device_name: Option<&str>) {
        if self.is_playing.load(Ordering::Relaxed) {
            return;
        }

        let buffer = self.mixed_buffer.clone();
        let sample_rate = self.sample_rate;
        let is_playing = self.is_playing.clone();
        let current_frame = self.current_frame.clone();
        let debug_buffer = buffer.clone(); // for debug WAV if play fails

        if buffer.is_empty() || sample_rate == 0 {
            tracing::warn!("AudioPlayback: buffer empty or sample_rate=0");
            return;
        }

        let channels = 2u16;
        tracing::info!(
            "AudioPlayback: starting with {} frames at {} Hz",
            buffer.len() / channels as usize,
            sample_rate
        );

        let result = cpal::default_host()
            .output_devices()
            .ok()
            .and_then(|mut devices| {
                // If a specific device is requested, try to find it
                let dev: Option<cpal::Device> = if let Some(name) = device_name {
                    devices.find(|d| d.name().ok().as_deref() == Some(name))
                } else {
                    devices.next()
                };
                if let Some(ref d) = dev {
                    if let Ok(name) = d.name() {
                        tracing::info!("AudioPlayback: using device: {}", name);
                    }
                } else {
                    tracing::error!(
                        "AudioPlayback: no output device found (requested: {:?})",
                        device_name
                    );
                }
                dev
            })
            .and_then(|device| {
                // Use our buffer's sample rate explicitly instead of device default
                let config = cpal::StreamConfig {
                    channels,
                    sample_rate: cpal::SampleRate(sample_rate),
                    buffer_size: cpal::BufferSize::Default,
                };
                tracing::info!(
                    "AudioPlayback: config = {} Hz, {} ch",
                    config.sample_rate.0,
                    config.channels
                );

                let err_fn = |err: cpal::StreamError| {
                    tracing::error!("AudioPlayback stream error: {:?}", err);
                };

                let stream_result = device.build_output_stream(
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

                        if frames_avail == 0 {
                            data.fill(0.0);
                            return;
                        }

                        let start_frame = current_frame.load(Ordering::Relaxed) as usize;
                        let mut out_idx = 0;
                        let mut write_frame = start_frame;

                        while out_idx < data.len() {
                            if write_frame >= frames_avail {
                                data[out_idx..].fill(0.0);
                                break;
                            }
                            let buf_idx = write_frame * channels as usize;
                            let samples_to_end = (frames_avail - write_frame) * channels as usize;
                            let remaining_out = data.len() - out_idx;
                            let copy = samples_to_end.min(remaining_out);

                            if buf_idx + copy <= buffer.len() {
                                data[out_idx..out_idx + copy]
                                    .copy_from_slice(&buffer[buf_idx..buf_idx + copy]);
                            }
                            out_idx += copy;
                            write_frame += copy / channels as usize;
                        }

                        current_frame.store(write_frame as u64, Ordering::Relaxed);
                    },
                    err_fn,
                    None,
                );

                match stream_result {
                    Ok(s) => {
                        tracing::info!("AudioPlayback: output stream built successfully");
                        Some(s)
                    }
                    Err(e) => {
                        tracing::error!("AudioPlayback: build_output_stream error: {:?}", e);
                        None
                    }
                }
            });

        if let Some(stream) = result {
            if let Err(e) = stream.play() {
                tracing::error!("AudioPlayback: stream.play() failed: {:?}", e);
            } else {
                tracing::info!("AudioPlayback: stream.play() OK");
            }
            self.stream = Some(stream);
            self.is_playing.store(true, Ordering::Relaxed);
            tracing::info!("AudioPlayback: started successfully");
        } else {
            tracing::error!("AudioPlayback: FAILED TO START - no stream created");
            // Try writing to a test WAV to verify PCM data exists
            if let Ok(cwd) = std::env::current_dir() {
                let test_path = cwd.join("_audio_debug_test.wav");
                if let Ok(mut w) = hound::WavWriter::create(
                    &test_path,
                    hound::WavSpec {
                        channels,
                        sample_rate,
                        bits_per_sample: 32,
                        sample_format: hound::SampleFormat::Float,
                    },
                ) {
                    for &s in debug_buffer.iter().take(48000 * 2 * 5) {
                        // first 5 seconds
                        let _ = w.write_sample(s);
                    }
                    let _ = w.finalize();
                    tracing::info!("AudioPlayback: wrote test WAV to {:?}", test_path);
                }
            }
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
