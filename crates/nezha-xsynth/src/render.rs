use std::path::Path;
use std::sync::Arc;

use nezha_core::{MidiControlEvent, MidiFile};
use xsynth_core::{
    AudioPipe, AudioStreamParams, ChannelCount,
    channel::{ChannelAudioEvent, ChannelConfigEvent, ChannelEvent, ControlEvent},
    channel_group::{
        ChannelGroup, ChannelGroupConfig, ParallelismOptions, SynthEvent, SynthFormat,
    },
    soundfont::{SampleSoundfont, SoundfontBase, SoundfontInitOptions},
};

/// The total number of synth channels: 16 ports × 16 MIDI channels.
pub const TOTAL_CHANNELS: u32 = 256;

/// Configuration for offline audio rendering.
#[derive(Clone, Debug)]
pub struct RenderConfig {
    pub sample_rate: u32,
    pub channels: ChannelCount,
    pub use_limiter: bool,
    pub layers: Option<usize>,
    /// 筛除所有力度 ≤ 此值的音符（默认 1 表示只筛掉力度 0 和 1）。
    pub min_velocity: u8,
    /// 每批渲染的样本数。更大 = 更少 xsynth 调用开销，
    /// 但事件会在 block 边界被批量 flush，导致时间量化误差。
    /// 默认 512（~10ms @ 48kHz），人耳听不出误差。
    /// 不建议超过 1024，否则密集 MIDI 可能出现节奏错乱。
    pub render_block_samples: usize,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: ChannelCount::Stereo,
            use_limiter: true,
            layers: Some(32),
            min_velocity: 1,
            render_block_samples: 512,
        }
    }
}

/// Progress reported during rendering.
#[derive(Clone, Debug)]
pub struct RenderProgress {
    pub elapsed_seconds: f64,
    pub total_seconds: f64,
    pub voice_count: u64,
}

/// Errors that can occur during rendering.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("MIDI load error: {0}")]
    MidiLoad(#[from] nezha_core::MidiError),

    #[error("SoundFont load error: {0}")]
    SoundfontLoad(String),

    #[error("WAV write error: {0}")]
    WavWrite(#[from] hound::Error),

    #[error("xsynth error: {0}")]
    Xsynth(String),
}

// ── Internal MIDI event types ──

#[derive(Clone, Debug)]
enum SynthCommand {
    NoteOn {
        key: u8,
        vel: u8,
        channel: u32,
    },
    NoteOff {
        key: u8,
        channel: u32,
    },
    ControlChange {
        controller: u8,
        value: u8,
        channel: u32,
    },
    ProgramChange {
        program: u8,
        channel: u32,
    },
    PitchBend {
        value: i16,
        channel: u32,
    },
}

struct TimedEvent {
    time_sec: f64,
    command: SynthCommand,
}

// ── Helpers ──

fn send_command(channel_group: &mut ChannelGroup, cmd: &SynthCommand) {
    match cmd {
        SynthCommand::NoteOn { key, vel, channel } => {
            channel_group.send_event(SynthEvent::Channel(
                *channel,
                ChannelEvent::Audio(ChannelAudioEvent::NoteOn {
                    key: *key,
                    vel: *vel,
                }),
            ));
        }
        SynthCommand::NoteOff { key, channel } => {
            channel_group.send_event(SynthEvent::Channel(
                *channel,
                ChannelEvent::Audio(ChannelAudioEvent::NoteOff { key: *key }),
            ));
        }
        SynthCommand::ControlChange {
            controller,
            value,
            channel,
        } => {
            channel_group.send_event(SynthEvent::Channel(
                *channel,
                ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::Raw(
                    *controller,
                    *value,
                ))),
            ));
        }
        SynthCommand::ProgramChange { program, channel } => {
            channel_group.send_event(SynthEvent::Channel(
                *channel,
                ChannelEvent::Audio(ChannelAudioEvent::ProgramChange(*program)),
            ));
        }
        SynthCommand::PitchBend { value, channel } => {
            let normalized = *value as f32 / 8192.0;
            channel_group.send_event(SynthEvent::Channel(
                *channel,
                ChannelEvent::Audio(ChannelAudioEvent::Control(ControlEvent::PitchBendValue(
                    normalized,
                ))),
            ));
        }
    }
}

fn render_samples(
    channel_group: &mut ChannelGroup,
    delta_sec: f64,
    config: &RenderConfig,
    scratch: &mut Vec<f32>,
    missed_samples: &mut f64,
    output: &mut Vec<f32>,
) {
    if delta_sec <= 0.0 {
        return;
    }

    let ch_count = config.channels.count() as usize;
    let sample_count_f = config.sample_rate as f64 * delta_sec + *missed_samples;
    *missed_samples = sample_count_f % 1.0;
    let samples_needed = (sample_count_f as usize) * ch_count;

    if samples_needed == 0 {
        return;
    }

    // Only grow the scratch buffer; never shrink.
    // This avoids repeated re-allocation across many small render calls.
    if scratch.len() < samples_needed {
        scratch.resize(samples_needed, 0.0);
    }
    let buf = &mut scratch[..samples_needed];
    channel_group.read_samples(buf);
    output.extend_from_slice(buf);
}

// ── MIDI event extraction from MidiFile ──

/// Extract timed events from a parsed [`MidiFile`] with port-aware channel mapping.
///
/// Returns `(events, total_seconds, max_channel)` where `max_channel`
/// is the highest channel number used (port-aware).
fn extract_timed_events(midi: &MidiFile, min_velocity: u8) -> (Vec<TimedEvent>, f64, u32) {
    let mut all_events: Vec<TimedEvent> = Vec::new();
    let mut max_channel: u32 = 0;

    // Note events from key_notes (already converted to seconds by nezha-core)
    for notes in &midi.key_notes {
        for note in notes {
            if note.velocity <= min_velocity {
                continue;
            }
            let ch = note.channel as u32;
            if ch > max_channel {
                max_channel = ch;
            }
            all_events.push(TimedEvent {
                time_sec: note.start,
                command: SynthCommand::NoteOn {
                    key: note.key,
                    vel: note.velocity,
                    channel: ch,
                },
            });
            all_events.push(TimedEvent {
                time_sec: note.end,
                command: SynthCommand::NoteOff {
                    key: note.key,
                    channel: ch,
                },
            });
        }
    }

    // Control events (CC, Program Change, Pitch Bend) — convert tick to seconds
    for evt in &midi.control_events {
        let (tick, ch, cmd) = match *evt {
            MidiControlEvent::ControlChange {
                tick,
                channel,
                controller,
                value,
            } => {
                let ch = channel as u32;
                (
                    tick,
                    ch,
                    SynthCommand::ControlChange {
                        controller,
                        value,
                        channel: ch,
                    },
                )
            }
            MidiControlEvent::ProgramChange {
                tick,
                channel,
                program,
            } => {
                let ch = channel as u32;
                (
                    tick,
                    ch,
                    SynthCommand::ProgramChange {
                        program,
                        channel: ch,
                    },
                )
            }
            MidiControlEvent::PitchBend {
                tick,
                channel,
                value,
            } => {
                let ch = channel as u32;
                (
                    tick,
                    ch,
                    SynthCommand::PitchBend {
                        value,
                        channel: ch,
                    },
                )
            }
        };
        if ch > max_channel {
            max_channel = ch;
        }
        all_events.push(TimedEvent {
            time_sec: midi.tick_to_seconds(tick),
            command: cmd,
        });
    }

    all_events.sort_by(|a, b| {
        a.time_sec
            .partial_cmp(&b.time_sec)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    (all_events, midi.duration, max_channel)
}

// ── Public API ──

/// Render MIDI to PCM with chunked output (supports incremental preview).
///
/// `on_chunk` is called periodically with (pcm_chunk, progress).
/// pcm_chunk contains the interleaved f32 samples rendered since the last call.
/// The caller should forward these to the main thread for playback.
pub fn render_midi_to_pcm_chunked(
    midi: &MidiFile,
    soundfont_paths: &[impl AsRef<Path>],
    config: &RenderConfig,
    mut on_chunk: impl FnMut(Vec<f32>, RenderProgress),
) -> Result<(), RenderError> {
    // 1. Extract events from parsed MidiFile
    let (events, total_seconds, max_channel) = extract_timed_events(midi, config.min_velocity);

    // 2. Load soundfonts
    let audio_params = AudioStreamParams::new(config.sample_rate, config.channels);

    let soundfonts: Vec<Arc<dyn SoundfontBase>> = soundfont_paths
        .iter()
        .map(|p| {
            let sf: Arc<dyn SoundfontBase> = Arc::new(
                SampleSoundfont::new(p.as_ref(), audio_params, SoundfontInitOptions::default())
                    .map_err(|e| RenderError::SoundfontLoad(format!("{:?}: {}", p.as_ref(), e)))?,
            );
            Ok(sf)
        })
        .collect::<Result<Vec<_>, RenderError>>()?;

    if soundfonts.is_empty() {
        return Err(RenderError::SoundfontLoad("No soundfonts provided".into()));
    }

    // 3. Create synth with only the channels actually used by the MIDI file.
    //    Always keep at least 16 channels so standard MIDI channel 9 (drums) exists.
    let channel_count = (max_channel + 1).max(16);
    let mut channel_group = ChannelGroup::new(ChannelGroupConfig {
        format: SynthFormat::Custom {
            channels: channel_count,
        },
        audio_params,
        channel_init_options: Default::default(),
        parallelism: ParallelismOptions::AUTO_PER_KEY,
    });

    // 4. Assign soundfonts to all created channels + percussion
    for ch in 0..channel_count {
        channel_group.send_event(SynthEvent::Channel(
            ch,
            ChannelEvent::Config(ChannelConfigEvent::SetSoundfonts(soundfonts.clone())),
        ));
        if let Some(layers) = config.layers {
            channel_group.send_event(SynthEvent::Channel(
                ch,
                ChannelEvent::Config(ChannelConfigEvent::SetLayerCount(Some(layers))),
            ));
        }
        if ch % 16 == 9 {
            channel_group.send_event(SynthEvent::Channel(
                ch,
                ChannelEvent::Config(ChannelConfigEvent::SetPercussionMode(true)),
            ));
        }
    }

    // 5. Render in fixed-size time blocks (windowed event batching)
    let mut pcm_buffer: Vec<f32> = Vec::new();
    let mut scratch: Vec<f32> = Vec::new();
    let mut missed_samples: f64 = 0.0;

    const CHUNK_INTERVAL_SECS: f64 = 0.5;
    let mut next_chunk_at: f64 = CHUNK_INTERVAL_SECS;

    let block_sec = config.render_block_samples as f64 / config.sample_rate as f64;

    let events_end_time = if events.is_empty() {
        0.0
    } else {
        events.last().unwrap().time_sec
    };

    let mut block_start = 0.0_f64;
    let mut event_idx = 0;

    while block_start < events_end_time {
        let block_end = (block_start + block_sec).min(events_end_time);
        let delta = block_end - block_start;

        while event_idx < events.len() && events[event_idx].time_sec < block_end {
            send_command(&mut channel_group, &events[event_idx].command);
            event_idx += 1;
        }

        render_samples(
            &mut channel_group,
            delta,
            config,
            &mut scratch,
            &mut missed_samples,
            &mut pcm_buffer,
        );

        if block_end >= next_chunk_at {
            let chunk = std::mem::take(&mut pcm_buffer);
            let progress = RenderProgress {
                elapsed_seconds: block_end,
                total_seconds,
                voice_count: channel_group.voice_count(),
            };
            on_chunk(chunk, progress);
            next_chunk_at += CHUNK_INTERVAL_SECS;
        }

        block_start = block_end;
    }

    // Flush remaining PCM buffer after all events
    if !pcm_buffer.is_empty() || total_seconds > 0.0 {
        let chunk = std::mem::take(&mut pcm_buffer);
        let progress = RenderProgress {
            elapsed_seconds: events_end_time.max(total_seconds),
            total_seconds,
            voice_count: channel_group.voice_count(),
        };
        on_chunk(chunk, progress);
    }

    // 6. Release tail (let notes fade naturally)
    let release_time = 2.0;
    render_samples(
        &mut channel_group,
        release_time,
        config,
        &mut scratch,
        &mut missed_samples,
        &mut pcm_buffer,
    );

    // 7. All notes off
    channel_group.send_event(SynthEvent::AllChannels(ChannelEvent::Audio(
        ChannelAudioEvent::AllNotesOff,
    )));
    channel_group.send_event(SynthEvent::AllChannels(ChannelEvent::Audio(
        ChannelAudioEvent::ResetControl,
    )));

    // 8. Flush tail audio until silence (larger 2s batches)
    let mut tail_remaining = 5.0_f64;
    while tail_remaining > 0.0 {
        let delta = tail_remaining.min(2.0);
        let prev_output_len = pcm_buffer.len();
        render_samples(
            &mut channel_group,
            delta,
            config,
            &mut scratch,
            &mut missed_samples,
            &mut pcm_buffer,
        );
        tail_remaining -= delta;

        let new_samples = &pcm_buffer[prev_output_len..];
        if new_samples.iter().all(|s| s.abs() <= 0.0001) {
            break;
        }
    }

    // Final tail chunk
    if !pcm_buffer.is_empty() {
        let chunk = std::mem::take(&mut pcm_buffer);
        let progress = RenderProgress {
            elapsed_seconds: total_seconds,
            total_seconds,
            voice_count: channel_group.voice_count(),
        };
        on_chunk(chunk, progress);
    }

    Ok(())
}

/// Render MIDI to a complete PCM buffer (blocks until done).
pub fn render_midi_to_pcm(
    midi: &MidiFile,
    soundfont_paths: &[impl AsRef<Path>],
    config: &RenderConfig,
    mut progress: impl FnMut(RenderProgress),
) -> Result<Vec<f32>, RenderError> {
    let mut all_pcm: Vec<f32> = Vec::new();

    render_midi_to_pcm_chunked(midi, soundfont_paths, config, |chunk, p| {
        all_pcm.extend(chunk);
        progress(p);
    })?;

    // Apply proper lookahead limiter if enabled
    if config.use_limiter {
        let channels = config.channels.count() as usize;
        let mut limiter = crate::limiter::Limiter::new(
            config.sample_rate as f32,
            channels,
            -1.0,  // threshold: -1 dBFS
            -0.3,  // ceiling: -0.3 dBFS
            2.0,   // 2 ms lookahead
            0.5,   // 0.5 ms attack
            100.0, // 100 ms release
        );
        limiter.process(&mut all_pcm);
    }

    Ok(all_pcm)
}

/// Convenience: render MIDI to WAV file on disk.
pub fn render_midi_to_wav(
    midi: &MidiFile,
    soundfont_paths: &[impl AsRef<Path>],
    output_wav_path: impl AsRef<Path>,
    config: &RenderConfig,
    progress: impl FnMut(RenderProgress),
) -> Result<(), RenderError> {
    let pcm = render_midi_to_pcm(midi, soundfont_paths, config, progress)?;

    let spec = hound::WavSpec {
        channels: config.channels.count(),
        sample_rate: config.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(output_wav_path, spec)?;
    for sample in &pcm {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;
    Ok(())
}
