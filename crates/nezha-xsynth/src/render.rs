use std::path::Path;
use std::sync::Arc;

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
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
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: ChannelCount::Stereo,
            use_limiter: true,
            layers: Some(32),
            min_velocity: 1,
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
    #[error("MIDI parse error: {0}")]
    MidiParse(#[from] midly::Error),

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
    // Cap each batch to 10s to avoid huge allocations
    if delta_sec > 10.0 {
        let mut remaining = delta_sec;
        while remaining > 0.0 {
            let batch = remaining.min(10.0);
            render_samples(
                channel_group,
                batch,
                config,
                scratch,
                missed_samples,
                output,
            );
            remaining -= batch;
        }
        return;
    }

    let ch_count = config.channels.count() as usize;
    let sample_count_f = config.sample_rate as f64 * delta_sec + *missed_samples;
    *missed_samples = sample_count_f % 1.0;
    let samples_needed = (sample_count_f as usize) * ch_count;

    if samples_needed == 0 {
        return;
    }

    scratch.resize(samples_needed, 0.0);
    channel_group.read_samples(scratch);
    output.extend_from_slice(scratch);
}

// ── MIDI parsing ──

/// Parse a MIDI file and extract timed events with port-aware channel mapping.
fn parse_midi_events(
    data: &[u8],
    min_velocity: u8,
) -> Result<(Vec<TimedEvent>, f64, u32), RenderError> {
    let smf = Smf::parse(data)?;

    let ticks_per_beat = match smf.header.timing {
        Timing::Metrical(t) => t.as_int() as u32,
        _ => 480,
    };

    // Collect tempo events
    let mut tempo_events: Vec<(u32, f64)> = Vec::new();
    for track in &smf.tracks {
        let mut tick: u32 = 0;
        for event in track {
            tick += event.delta.as_int();
            if let TrackEventKind::Meta(MetaMessage::Tempo(us)) = event.kind {
                tempo_events.push((tick, us.as_int() as f64));
            }
        }
    }
    tempo_events.sort_by_key(|e| e.0);
    tempo_events.dedup_by_key(|e| e.0);

    const DEFAULT_MPQ: f64 = 500_000.0;
    if tempo_events.is_empty() || tempo_events[0].0 > 0 {
        tempo_events.insert(0, (0, DEFAULT_MPQ));
    }

    let tick_to_sec = |tick: u32| -> f64 {
        let mut sec = 0.0;
        let mut prev_tick = 0u32;
        let mut prev_mpq = DEFAULT_MPQ;
        for &(t, mpq) in &tempo_events {
            if t > tick {
                break;
            }
            if t > prev_tick {
                let dt = (t - prev_tick) as f64;
                sec += dt * prev_mpq / (ticks_per_beat as f64 * 1_000_000.0);
            }
            prev_tick = t;
            prev_mpq = mpq;
        }
        if tick > prev_tick {
            let dt = (tick - prev_tick) as f64;
            sec += dt * prev_mpq / (ticks_per_beat as f64 * 1_000_000.0);
        }
        sec
    };

    // Parse events per track with port tracking
    let mut all_events: Vec<TimedEvent> = Vec::new();
    let mut global_end_time = 0.0_f64;

    for track in &smf.tracks {
        let mut current_tick: u32 = 0;
        let mut current_port: u8 = 0;

        for event in track {
            current_tick += event.delta.as_int();

            match event.kind {
                TrackEventKind::Meta(MetaMessage::MidiPort(port)) => {
                    current_port = port.as_int();
                }
                TrackEventKind::Meta(MetaMessage::EndOfTrack) => {
                    let t = tick_to_sec(current_tick);
                    if t > global_end_time {
                        global_end_time = t;
                    }
                }
                TrackEventKind::Midi { channel, message } => {
                    let time_sec = tick_to_sec(current_tick);
                    let ch = current_port as u32 * 16 + channel.as_int() as u32;

                    let command = match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            if vel.as_int() <= min_velocity {
                                None
                            } else {
                                Some(SynthCommand::NoteOn {
                                    key: key.as_int(),
                                    vel: vel.as_int(),
                                    channel: ch,
                                })
                            }
                        }
                        MidiMessage::NoteOn { key, .. } => Some(SynthCommand::NoteOff {
                            key: key.as_int(),
                            channel: ch,
                        }),
                        MidiMessage::NoteOff { key, .. } => Some(SynthCommand::NoteOff {
                            key: key.as_int(),
                            channel: ch,
                        }),
                        MidiMessage::Controller { controller, value } => {
                            Some(SynthCommand::ControlChange {
                                controller: controller.as_int(),
                                value: value.as_int(),
                                channel: ch,
                            })
                        }
                        MidiMessage::ProgramChange { program } => {
                            Some(SynthCommand::ProgramChange {
                                program: program.as_int(),
                                channel: ch,
                            })
                        }
                        MidiMessage::PitchBend { bend } => Some(SynthCommand::PitchBend {
                            value: bend.as_int(),
                            channel: ch,
                        }),
                        _ => None,
                    };

                    if let Some(cmd) = command {
                        all_events.push(TimedEvent {
                            time_sec,
                            command: cmd,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    all_events.sort_by(|a, b| {
        a.time_sec
            .partial_cmp(&b.time_sec)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok((all_events, global_end_time, ticks_per_beat))
}

// ── Public API ──

/// Render MIDI to PCM with chunked output (supports incremental preview).
///
/// `on_chunk` is called periodically with (pcm_chunk, progress).
/// pcm_chunk contains the interleaved f32 samples rendered since the last call.
/// The caller should forward these to the main thread for playback.
pub fn render_midi_to_pcm_chunked(
    midi_data: &[u8],
    soundfont_paths: &[impl AsRef<Path>],
    config: &RenderConfig,
    mut on_chunk: impl FnMut(Vec<f32>, RenderProgress),
) -> Result<(), RenderError> {
    // 1. Parse MIDI
    let (events, total_seconds, _ticks_per_beat) =
        parse_midi_events(midi_data, config.min_velocity)?;

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

    // 3. Create synth with 256 channels
    let mut channel_group = ChannelGroup::new(ChannelGroupConfig {
        format: SynthFormat::Custom {
            channels: TOTAL_CHANNELS,
        },
        audio_params,
        channel_init_options: Default::default(),
        parallelism: ParallelismOptions::AUTO_PER_KEY,
    });

    // 4. Assign soundfonts to all channels + percussion
    for ch in 0..TOTAL_CHANNELS {
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

    // 5. Render in timestamp batches
    let _ch_count = audio_params.channels.count() as usize;
    let mut pcm_buffer: Vec<f32> = Vec::new();
    let mut scratch: Vec<f32> = Vec::new();
    let mut missed_samples: f64 = 0.0;

    // Group events by unique timestamp and process in batches
    let mut i = 0;
    let mut prev_time: f64 = 0.0;
    const CHUNK_INTERVAL_SECS: f64 = 3.0; // flush PCM chunk every 3 seconds
    let mut next_chunk_at: f64 = CHUNK_INTERVAL_SECS;

    let mut last_time = 0.0;

    while i < events.len() {
        let current_time = events[i].time_sec;
        last_time = current_time;

        // Collect all events at this exact timestamp
        let batch_end = (i + 1..events.len())
            .find(|&j| (events[j].time_sec - current_time).abs() > 1e-9)
            .unwrap_or(events.len());

        // 5a. Render audio for the time span since the last batch
        let delta = current_time - prev_time;
        render_samples(
            &mut channel_group,
            delta,
            config,
            &mut scratch,
            &mut missed_samples,
            &mut pcm_buffer,
        );
        prev_time = current_time;

        // 5b. Send all events in this batch
        for ev in &events[i..batch_end] {
            send_command(&mut channel_group, &ev.command);
        }

        // 5c. Emit chunk if we've accumulated enough audio
        if current_time >= next_chunk_at {
            let chunk = std::mem::take(&mut pcm_buffer);
            let progress = RenderProgress {
                elapsed_seconds: current_time,
                total_seconds,
                voice_count: channel_group.voice_count(),
            };
            on_chunk(chunk, progress);
            next_chunk_at += CHUNK_INTERVAL_SECS;
        }

        i = batch_end;
    }

    // 5d. Final progress report after all events
    if total_seconds > 0.0 {
        let progress = RenderProgress {
            elapsed_seconds: last_time,
            total_seconds,
            voice_count: channel_group.voice_count(),
        };
        // Flush remaining PCM buffer
        let chunk = std::mem::take(&mut pcm_buffer);
        on_chunk(chunk, progress);
    }

    // 6. Release tail (+ render remaining buffer)
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

    // Flush tail audio until silence
    let mut tail_current = 0.0_f64;
    while tail_current < 5.0 {
        let delta = (5.0 - tail_current).min(1.0);
        render_samples(
            &mut channel_group,
            delta,
            config,
            &mut scratch,
            &mut missed_samples,
            &mut pcm_buffer,
        );
        tail_current += delta;

        // Check for silence
        if scratch.iter().all(|s| s.abs() <= 0.0001) {
            break;
        }
    }

    // Final tail chunk
    let chunk = std::mem::take(&mut pcm_buffer);
    let progress = RenderProgress {
        elapsed_seconds: total_seconds,
        total_seconds,
        voice_count: channel_group.voice_count(),
    };
    on_chunk(chunk, progress);

    Ok(())
}

/// Render MIDI to a complete PCM buffer (blocks until done).
pub fn render_midi_to_pcm(
    midi_data: &[u8],
    soundfont_paths: &[impl AsRef<Path>],
    config: &RenderConfig,
    mut progress: impl FnMut(RenderProgress),
) -> Result<Vec<f32>, RenderError> {
    let mut all_pcm: Vec<f32> = Vec::new();

    render_midi_to_pcm_chunked(midi_data, soundfont_paths, config, |chunk, p| {
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
    midi_data: &[u8],
    soundfont_paths: &[impl AsRef<Path>],
    output_wav_path: impl AsRef<Path>,
    config: &RenderConfig,
    progress: impl FnMut(RenderProgress),
) -> Result<(), RenderError> {
    let pcm = render_midi_to_pcm(midi_data, soundfont_paths, config, progress)?;

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
