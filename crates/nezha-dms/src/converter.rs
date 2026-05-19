use crate::DmsError;
use crate::model::{DmsDocument, RawMidiEventKind};
use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Timing, TrackEvent, TrackEventKind};

/// 将 `DmsDocument` 序列化为标准 SMF 字节流。
pub(crate) fn to_smf_bytes(doc: &DmsDocument) -> Result<Vec<u8>, DmsError> {
    let mut leaked: Vec<&'static [u8]> = Vec::new();
    let mut leak = |s: String| -> &'static [u8] {
        let b: &'static [u8] = Box::leak(s.into_bytes().into_boxed_slice());
        leaked.push(b);
        b
    };

    let mut smf_tracks: Vec<Vec<TrackEvent<'static>>> = Vec::new();

    for track in &doc.tracks {
        let mut events = track.events.clone();
        events.sort_by_key(|e| e.tick);

        let mut midi_events: Vec<TrackEvent<'static>> = Vec::new();
        let mut last_tick: u32 = 0;

        for ev in &events {
            let delta = ev.tick.saturating_sub(last_tick);
            last_tick = ev.tick;
            let delta = u28::from(delta);

            let kind = match &ev.kind {
                RawMidiEventKind::NoteOn { key, vel, channel } => TrackEventKind::Midi {
                    channel: u4::from_int_lossy(*channel),
                    message: MidiMessage::NoteOn {
                        key: u7::from_int_lossy(*key),
                        vel: u7::from_int_lossy(*vel),
                    },
                },
                RawMidiEventKind::NoteOff { key, channel } => TrackEventKind::Midi {
                    channel: u4::from_int_lossy(*channel),
                    message: MidiMessage::NoteOff {
                        key: u7::from_int_lossy(*key),
                        vel: u7::from_int_lossy(0),
                    },
                },
                RawMidiEventKind::Tempo { bpm, base_gate } => {
                    // 基准 Gate 决定速度缩放：base_gate == PPQ 时原速。
                    // base_gate 越大（参考音符越长）→ 实际速度越快，
                    // 因为同样的 BPM 值覆盖的是更长的参考音符。
                    // 例如 BPM100、基准 Gate960(=2×PPQ) → 每分钟100个"2分音符"
                    // = 每分钟200个四分音符 → adjusted BPM = 200。
                    let effective_gate = if *base_gate == 0 {
                        doc.ppqn as f64
                    } else {
                        *base_gate as f64
                    };
                    let adjusted_bpm = *bpm * (effective_gate / doc.ppqn as f64);
                    let mpq = (60_000_000.0 / adjusted_bpm).clamp(1.0, 16_777_215.0) as u32;
                    TrackEventKind::Meta(MetaMessage::Tempo(u24::from(mpq)))
                }
                RawMidiEventKind::TimeSig { num, den } => {
                    let log2_den = den
                        .checked_next_power_of_two()
                        .map(|v| v.trailing_zeros() as u8)
                        .unwrap_or(2);
                    TrackEventKind::Meta(MetaMessage::TimeSignature(*num, log2_den, 24, 8))
                }
                RawMidiEventKind::KeySig { sf, mi } => {
                    TrackEventKind::Meta(MetaMessage::KeySignature(*sf, *mi != 0))
                }
                RawMidiEventKind::Control { cc, value, channel } => TrackEventKind::Midi {
                    channel: u4::from_int_lossy(*channel),
                    message: MidiMessage::Controller {
                        controller: u7::from_int_lossy(*cc),
                        value: u7::from_int_lossy(*value),
                    },
                },
                RawMidiEventKind::ProgramChange { program, channel } => TrackEventKind::Midi {
                    channel: u4::from_int_lossy(*channel),
                    message: MidiMessage::ProgramChange {
                        program: u7::from_int_lossy(*program),
                    },
                },
                RawMidiEventKind::TrackName { name } => {
                    TrackEventKind::Meta(MetaMessage::TrackName(leak(name.clone())))
                }
                RawMidiEventKind::Lyric { text } => {
                    TrackEventKind::Meta(MetaMessage::Lyric(leak(text.clone())))
                }
                RawMidiEventKind::Marker { name } => {
                    TrackEventKind::Meta(MetaMessage::Marker(leak(name.clone())))
                }
                RawMidiEventKind::EndOfTrack => TrackEventKind::Meta(MetaMessage::EndOfTrack),
            };

            midi_events.push(TrackEvent { delta, kind });
        }

        // 确保有 EndOfTrack
        if !midi_events
            .iter()
            .any(|e| matches!(e.kind, TrackEventKind::Meta(MetaMessage::EndOfTrack)))
        {
            midi_events.push(TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            });
        }

        smf_tracks.push(midi_events);
    }

    // 如果没有 track，创建一个空的
    if smf_tracks.is_empty() {
        smf_tracks.push(vec![TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        }]);
    }

    let header = Header {
        format: Format::Parallel,
        timing: Timing::Metrical(u15::from_int_lossy(doc.ppqn as u16)),
    };

    let mut buf = Vec::new();
    let track_iters: Vec<_> = smf_tracks.iter().map(|t| t.iter()).collect();
    midly::write_std(&header, track_iters.into_iter(), &mut buf)
        .map_err(|e| DmsError::MidiConvert(e.to_string()))?;

    Ok(buf)
}
