use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::app::project_state::AudioStore;

pub struct AudioPlayback {
    pub is_playing: Arc<AtomicBool>,
    pub current_frame: Arc<AtomicU64>,
    mixed_buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
    stream: Option<cpal::Stream>,
    device_name: Option<String>,
    stream_initialized: bool,
}

impl AudioPlayback {
    pub fn new() -> Self {
        Self {
            is_playing: Arc::new(AtomicBool::new(false)),
            current_frame: Arc::new(AtomicU64::new(0)),
            mixed_buffer: Arc::new(Mutex::new(Vec::new())),
            sample_rate: 48000,
            channels: 2,
            stream: None,
            device_name: None,
            stream_initialized: false,
        }
    }

    /// Set the preferred device name. The stream will be (re)created on next play().
    pub fn set_device(&mut self, name: Option<String>) {
        if self.device_name != name {
            self.device_name = name;
            self.stream = None; // force re-init on next play
            self.stream_initialized = false;
        }
    }

    pub fn mix(
        &mut self,
        audio_store: &AudioStore,
        timeline_clips: &[(usize, f32, f32)],
        duration_secs: f64,
        sample_rate: u32,
    ) {
        let mixed = audio_store.mix_master(timeline_clips, sample_rate, duration_secs);
        if let Ok(mut buf) = self.mixed_buffer.lock() {
            *buf = mixed;
        }
        self.sample_rate = sample_rate;
    }

    /// Start playback. Lazily creates the cpal stream on first call.
    pub fn play(&mut self, start_frame: u64) {
        if !self.stream_initialized {
            self.init_stream();
        }
        self.current_frame.store(start_frame, Ordering::SeqCst);
        self.is_playing.store(true, Ordering::SeqCst);
        if let Some(ref stream) = self.stream {
            let _ = stream.play();
        }
    }

    fn init_stream(&mut self) {
        self.stream = None;
        self.is_playing.store(false, Ordering::Relaxed);
        self.current_frame.store(0, Ordering::Relaxed);

        let buffer = self.mixed_buffer.clone();
        let sample_rate = self.sample_rate;
        let channels = self.channels;
        let is_playing = self.is_playing.clone();
        let current_frame = self.current_frame.clone();

        let device_name = self.device_name.clone();

        let result = cpal::default_host()
            .output_devices()
            .ok()
            .and_then(|mut devices| {
                // Try to find the preferred device, or fall back to default
                let dev: Option<cpal::Device> = if let Some(ref name) = device_name {
                    let found = devices.find(|d| d.name().ok().as_deref() == Some(name.as_str()));
                    if found.is_some() {
                        found
                    } else {
                        // Fallback: re-enumerate and take first
                        cpal::default_host()
                            .output_devices()
                            .ok()
                            .and_then(|mut ds| ds.next())
                    }
                } else {
                    devices.next()
                };
                if let Some(ref d) = dev {
                    if let Ok(name) = d.name() {
                        tracing::info!("AudioPlayback: using device: {}", name);
                    }
                } else {
                    tracing::error!("AudioPlayback: no output device found");
                }
                dev
            })
            .and_then(|device| {
                let config = cpal::StreamConfig {
                    channels,
                    sample_rate: cpal::SampleRate(sample_rate),
                    buffer_size: cpal::BufferSize::Default,
                };

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

                        let buf = match buffer.lock() {
                            Ok(b) => b,
                            Err(_) => {
                                data.fill(0.0);
                                return;
                            }
                        };

                        let buf_len = buf.len();
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

                            if buf_idx + copy <= buf.len() {
                                data[out_idx..out_idx + copy]
                                    .copy_from_slice(&buf[buf_idx..buf_idx + copy]);
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
                        tracing::info!("AudioPlayback: stream built OK");
                        if let Err(e) = s.play() {
                            tracing::error!("AudioPlayback: stream.play() failed: {:?}", e);
                        }
                        Some(s)
                    }
                    Err(e) => {
                        tracing::error!("AudioPlayback: build_output_stream error: {:?}", e);
                        None
                    }
                }
            });

        if let Some(stream) = result {
            self.stream = Some(stream);
            self.stream_initialized = true;
            tracing::info!("AudioPlayback: initialized successfully");
        } else {
            tracing::error!("AudioPlayback: FAILED to initialize stream");
        }
    }

    pub fn pause(&self) {
        self.is_playing.store(false, Ordering::SeqCst);
    }

    #[expect(dead_code)]
    pub fn stop(&mut self) {
        self.pause();
        self.current_frame.store(0, Ordering::Relaxed);
    }

    pub fn seek_to(&self, time_sec: f64) {
        if self.sample_rate > 0 {
            let frame = (time_sec * self.sample_rate as f64) as u64;
            self.current_frame.store(frame, Ordering::SeqCst);
        }
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    /// Returns a clone of the internal mixed buffer Arc, allowing external
    /// threads (e.g. a background mixer) to write into it directly.
    pub fn mixed_buffer_arc(&self) -> Arc<Mutex<Vec<f32>>> {
        self.mixed_buffer.clone()
    }

    pub fn buffer_len(&self) -> usize {
        self.mixed_buffer.lock().map(|b| b.len()).unwrap_or(0)
    }

    #[expect(dead_code)]
    pub fn current_time(&self) -> f64 {
        if self.sample_rate > 0 {
            self.current_frame.load(Ordering::Relaxed) as f64 / self.sample_rate as f64
        } else {
            0.0
        }
    }
}
