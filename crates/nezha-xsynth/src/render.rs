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

// ── MIDI parsing ──

/// Parse a MIDI file and extract timed events with port-aware channel mapping.
///
/// Returns `(events, total_seconds, ticks_per_beat, max_channel)` where `max_channel`
/// is the highest channel number used in the MIDI file (0-based, port-aware).
fn parse_midi_events(
    data: &[u8],
    min_velocity: u8,
) -> Result<(Vec<TimedEvent>, f64, u32, u32), RenderError> {
    let smf = Smf::parse(data)?;

    let ticks_per_beat = match smf.header.timing {
        Timing::Metrical(t) => t.as_int() as u32,
        _ => 480,
    };

    // Collect tempo events and find max tick across all tracks
    let mut max_tick = 0u32;
    let mut tempo_events: Vec<(u32, f64)> = Vec::new();
    for track in &smf.tracks {
        let mut tick: u32 = 0;
        for event in track {
            tick += event.delta.as_int();
            if tick > max_tick {
                max_tick = tick;
            }
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

    // Pre-compute tick→sec lookup table for O(1) conversion
    // (was O(n) linear scan per lookup)
    let mut tick_to_sec_table = Vec::with_capacity(max_tick as usize + 1);
    {
        let mut sec = 0.0f64;
        let mut prev_tempo_tick = 0u32;
        let mut prev_mpq = DEFAULT_MPQ;
        let mut tempo_idx = 0;

        for tick in 0..=max_tick {
            while tempo_idx < tempo_events.len() && tempo_events[tempo_idx].0 <= tick {
                let (tt, mpq) = tempo_events[tempo_idx];
                if tt > prev_tempo_tick {
                    let dt = (tt - prev_tempo_tick) as f64;
                    sec += dt * prev_mpq / (ticks_per_beat as f64 * 1_000_000.0);
                }
                prev_tempo_tick = tt;
                prev_mpq = mpq;
                tempo_idx += 1;
            }
            let time_at_tick = sec
                + (tick - prev_tempo_tick) as f64 * prev_mpq
                    / (ticks_per_beat as f64 * 1_000_000.0);
            tick_to_sec_table.push(time_at_tick);
        }
    }

    let tick_to_sec = |tick: u32| -> f64 { tick_to_sec_table[tick as usize] };

    // Parse events per track with port tracking
    let mut all_events: Vec<TimedEvent> = Vec::new();
    let mut global_end_time = 0.0_f64;
    let mut max_channel: u32 = 0;

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
                    if ch > max_channel {
                        max_channel = ch;
                    }

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

    Ok((all_events, global_end_time, ticks_per_beat, max_channel))
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
    let (events, total_seconds, _ticks_per_beat, max_channel) =
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
    //
    // Instead of calling render_samples for every unique event timestamp
    // (which can be tens/hundreds of thousands of calls for dense MIDI),
    // we process in configurable-size blocks, accumulating all events
    // within each block.  This drastically reduces the number of xsynth
    // read_samples() calls.
    let mut pcm_buffer: Vec<f32> = Vec::new();
    let mut scratch: Vec<f32> = Vec::new();
    let mut missed_samples: f64 = 0.0;

    const CHUNK_INTERVAL_SECS: f64 = 0.5; // flush PCM chunk every 0.5 seconds
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

        // Dispatch all events falling within this block FIRST so that
        // xsynth sees them before we call read_samples().  This avoids
        // delaying every event by one full block.
        while event_idx < events.len() && events[event_idx].time_sec < block_end {
            send_command(&mut channel_group, &events[event_idx].command);
            event_idx += 1;
        }

        // Render audio for this block (events are now cached inside xsynth)
        render_samples(
            &mut channel_group,
            delta,
            config,
            &mut scratch,
            &mut missed_samples,
            &mut pcm_buffer,
        );

        // Emit chunk if we've accumulated enough audio
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

        // Check newly rendered samples for silence
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
