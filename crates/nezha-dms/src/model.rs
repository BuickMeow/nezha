use crate::DmsError;
use crate::parser::decompress;
use crate::parser::{
    DmsNode, NODE_ABS_TICK_POS, NODE_CONTROL_EVENT, NODE_CONTROL_TYPE, NODE_CONTROL_VALUE,
    NODE_END_OF_TRACK_EVENT, NODE_KEY_SIG_EVENT, NODE_KEY_SIG_INDEX, NODE_LYRICS_EVENT,
    NODE_LYRICS_LYRICS, NODE_MARKER_EVENT, NODE_MARKER_NAME, NODE_NOTE_EVENT, NODE_NOTE_GATE,
    NODE_NOTE_KEY_NUMBER, NODE_NOTE_VELOCITY, NODE_PROGRAM_CHANGE_EVENT, NODE_SONG_PPQN,
    NODE_TEMPO_BASE_GATE, NODE_TEMPO_EVENT, NODE_TEMPO_VALUE, NODE_TIME_SIG_DENOMINATOR,
    NODE_TIME_SIG_EVENT, NODE_TIME_SIG_NUMERATOR, NODE_TRACK, NODE_TRACK_CHANNEL, NODE_TRACK_NAME,
    parse_float, parse_gbk_string, parse_integer, parse_root,
};

#[derive(Debug)]
pub(crate) struct DmsDocument {
    pub ppqn: u32,
    pub tracks: Vec<DmsTrack>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct DmsTrack {
    pub channel: u8,
    pub name: String,
    pub events: Vec<RawMidiEvent>,
}

#[derive(Debug, Clone)]
pub(crate) struct RawMidiEvent {
    pub tick: u32,
    pub kind: RawMidiEventKind,
}

#[derive(Debug, Clone)]
pub(crate) enum RawMidiEventKind {
    NoteOn { key: u8, vel: u8, channel: u8 },
    NoteOff { key: u8, channel: u8 },
    Tempo { bpm: f64, base_gate: u32 },
    TimeSig { num: u8, den: u8 },
    KeySig { sf: i8, mi: u8 },
    Control { cc: u8, value: u8, channel: u8 },
    ProgramChange { program: u8, channel: u8 },
    TrackName { name: String },
    Lyric { text: String },
    Marker { name: String },
    EndOfTrack,
}

impl DmsDocument {
    pub fn parse(data: &[u8]) -> Result<Self, DmsError> {
        let raw = decompress(data)?;
        let root = parse_root(&raw)?;

        let mut ppqn: u32 = 480;
        let mut tracks = Vec::new();

        for child in &root.children {
            match child.computed_type {
                NODE_SONG_PPQN => {
                    ppqn = parse_integer(&child.data) as u32;
                }
                NODE_TRACK => {
                    if let Some(track) = Self::parse_track(child) {
                        tracks.push(track);
                    }
                }
                _ => {}
            }
        }

        Ok(DmsDocument { ppqn, tracks })
    }

    fn parse_track(node: &DmsNode) -> Option<DmsTrack> {
        let mut channel: u8 = 0;
        let mut name = String::new();
        let mut events = Vec::new();

        for child in &node.children {
            match child.computed_type {
                NODE_TRACK_CHANNEL => {
                    channel = parse_integer(&child.data) as u8;
                }
                NODE_TRACK_NAME => {
                    name = parse_gbk_string(&child.data);
                }
                NODE_NOTE_EVENT => {
                    events.extend(Self::parse_note_event(child, channel));
                }
                NODE_TEMPO_EVENT => {
                    if let Some(ev) = Self::parse_tempo_event(child) {
                        events.push(ev);
                    }
                }
                NODE_TIME_SIG_EVENT => {
                    if let Some(ev) = Self::parse_time_sig_event(child) {
                        events.push(ev);
                    }
                }
                NODE_KEY_SIG_EVENT => {
                    if let Some(ev) = Self::parse_key_sig_event(child) {
                        events.push(ev);
                    }
                }
                NODE_CONTROL_EVENT => {
                    if let Some(ev) = Self::parse_control_event(child, channel) {
                        events.push(ev);
                    }
                }
                NODE_PROGRAM_CHANGE_EVENT => {
                    if let Some(ev) = Self::parse_program_change_event(child, channel) {
                        events.push(ev);
                    }
                }
                NODE_LYRICS_EVENT => {
                    if let Some(ev) = Self::parse_lyrics_event(child) {
                        events.push(ev);
                    }
                }
                NODE_MARKER_EVENT => {
                    if let Some(ev) = Self::parse_marker_event(child) {
                        events.push(ev);
                    }
                }
                NODE_END_OF_TRACK_EVENT => {
                    if let Some(tick) = Self::parse_abs_tick(child) {
                        events.push(RawMidiEvent {
                            tick,
                            kind: RawMidiEventKind::EndOfTrack,
                        });
                    }
                }
                _ => {}
            }
        }

        events.push(RawMidiEvent {
            tick: 0,
            kind: RawMidiEventKind::TrackName { name: name.clone() },
        });

        Some(DmsTrack {
            channel,
            name,
            events,
        })
    }

    fn parse_note_event(node: &DmsNode, channel: u8) -> Vec<RawMidiEvent> {
        let Some(tick) = Self::parse_abs_tick(node) else {
            return Vec::new();
        };
        let mut key: u8 = 60;
        let mut vel: u8 = 100;
        let mut gate: u32 = 480;

        for child in &node.children {
            match child.computed_type {
                NODE_NOTE_KEY_NUMBER => key = parse_integer(&child.data) as u8,
                NODE_NOTE_VELOCITY => vel = parse_integer(&child.data) as u8,
                NODE_NOTE_GATE => gate = parse_integer(&child.data) as u32,
                _ => {}
            }
        }

        vec![
            RawMidiEvent {
                tick,
                kind: RawMidiEventKind::NoteOn { key, vel, channel },
            },
            RawMidiEvent {
                tick: tick + gate,
                kind: RawMidiEventKind::NoteOff { key, channel },
            },
        ]
    }

    fn parse_tempo_event(node: &DmsNode) -> Option<RawMidiEvent> {
        let tick = Self::parse_abs_tick(node)?;
        let mut bpm = 120.0;
        let mut base_gate: u32 = 0; // 0 表示“未指定”，后续用 PPQ 作为默认值
        for child in &node.children {
            match child.computed_type {
                NODE_TEMPO_VALUE => bpm = parse_float(&child.data)?,
                NODE_TEMPO_BASE_GATE => {
                    if let Some(v) = parse_float(&child.data) {
                        base_gate = v as u32;
                    }
                }
                _ => {}
            }
        }
        Some(RawMidiEvent {
            tick,
            kind: RawMidiEventKind::Tempo { bpm, base_gate },
        })
    }

    fn parse_time_sig_event(node: &DmsNode) -> Option<RawMidiEvent> {
        let tick = Self::parse_abs_tick(node)?;
        let mut num: u8 = 4;
        let mut den: u8 = 4;
        for child in &node.children {
            match child.computed_type {
                NODE_TIME_SIG_NUMERATOR => num = parse_integer(&child.data) as u8,
                NODE_TIME_SIG_DENOMINATOR => den = parse_integer(&child.data) as u8,
                _ => {}
            }
        }
        Some(RawMidiEvent {
            tick,
            kind: RawMidiEventKind::TimeSig { num, den },
        })
    }

    fn parse_key_sig_event(node: &DmsNode) -> Option<RawMidiEvent> {
        let tick = Self::parse_abs_tick(node)?;
        let mut index: i64 = 0;
        for child in &node.children {
            if child.computed_type == NODE_KEY_SIG_INDEX {
                index = parse_integer(&child.data);
            }
        }
        let sf = index.clamp(-7, 7) as i8;
        let mi = 0u8;
        Some(RawMidiEvent {
            tick,
            kind: RawMidiEventKind::KeySig { sf, mi },
        })
    }

    fn parse_control_event(node: &DmsNode, channel: u8) -> Option<RawMidiEvent> {
        let tick = Self::parse_abs_tick(node)?;
        let mut cc: u8 = 0;
        let mut value: u8 = 0;
        for child in &node.children {
            match child.computed_type {
                NODE_CONTROL_TYPE => cc = parse_integer(&child.data) as u8,
                NODE_CONTROL_VALUE => {
                    if let Some(v) = parse_float(&child.data) {
                        value = v.clamp(0.0, 127.0) as u8;
                    }
                }
                _ => {}
            }
        }
        Some(RawMidiEvent {
            tick,
            kind: RawMidiEventKind::Control { cc, value, channel },
        })
    }

    fn parse_program_change_event(node: &DmsNode, channel: u8) -> Option<RawMidiEvent> {
        let tick = Self::parse_abs_tick(node)?;
        let mut program: u8 = 0;
        for child in &node.children {
            if !child.data.is_empty() && child.computed_type != NODE_ABS_TICK_POS {
                program = child.data.first().copied().unwrap_or(0);
            }
        }
        Some(RawMidiEvent {
            tick,
            kind: RawMidiEventKind::ProgramChange { program, channel },
        })
    }

    fn parse_lyrics_event(node: &DmsNode) -> Option<RawMidiEvent> {
        let tick = Self::parse_abs_tick(node)?;
        let mut text = String::new();
        for child in &node.children {
            if child.computed_type == NODE_LYRICS_LYRICS {
                text = parse_gbk_string(&child.data);
            }
        }
        Some(RawMidiEvent {
            tick,
            kind: RawMidiEventKind::Lyric { text },
        })
    }

    fn parse_marker_event(node: &DmsNode) -> Option<RawMidiEvent> {
        let tick = Self::parse_abs_tick(node)?;
        let mut name = String::new();
        for child in &node.children {
            if child.computed_type == NODE_MARKER_NAME {
                name = parse_gbk_string(&child.data);
            }
        }
        Some(RawMidiEvent {
            tick,
            kind: RawMidiEventKind::Marker { name },
        })
    }

    fn parse_abs_tick(node: &DmsNode) -> Option<u32> {
        for child in &node.children {
            if child.computed_type == NODE_ABS_TICK_POS {
                return Some(parse_integer(&child.data) as u32);
            }
        }
        None
    }
}
