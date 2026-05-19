use std::io::{self, Read};
use std::path::Path;

// ------------------------------------------------------------------
// 错误类型
// ------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum DmsError {
    #[error("IO 错误: {0}")]
    Io(#[from] io::Error),
    #[error("无效的 DMS 文件")]
    InvalidDms,
    #[error("不支持的 DMS 特性: {0}")]
    Unsupported(String),
    #[error("MIDI 转换错误: {0}")]
    MidiConvert(String),
}

// ------------------------------------------------------------------
// 公开 API
// ------------------------------------------------------------------

pub struct DmsFile;

impl DmsFile {
    /// 读取 DMS 文件并转换为 `nezha_core::MidiFile`。
    pub fn load(path: impl AsRef<Path>) -> Result<nezha_core::MidiFile, DmsError> {
        let data = std::fs::read(path)?;
        Self::from_bytes(&data)
    }

    /// 从内存中的 DMS 数据转换为 `nezha_core::MidiFile`。
    pub fn from_bytes(data: &[u8]) -> Result<nezha_core::MidiFile, DmsError> {
        let doc = DmsDocument::parse(data)?;
        let midi_bytes = doc.to_smf_bytes()?;
        nezha_core::MidiFile::load_from_bytes(&midi_bytes)
            .map_err(|e| DmsError::MidiConvert(e.to_string()))
    }
}

// ------------------------------------------------------------------
// DMS 文档模型
// ------------------------------------------------------------------

struct DmsDocument {
    ppqn: u32,
    tracks: Vec<DmsTrack>,
}

struct DmsTrack {
    channel: u8,
    name: String,
    events: Vec<RawMidiEvent>,
}

#[derive(Debug, Clone)]
struct RawMidiEvent {
    tick: u32,
    kind: RawMidiEventKind,
}

#[derive(Debug, Clone)]
enum RawMidiEventKind {
    NoteOn { key: u8, vel: u8, channel: u8 },
    NoteOff { key: u8, channel: u8 },
    Tempo { bpm: f64 },
    TimeSig { num: u8, den: u8 },
    KeySig { sf: i8, mi: u8 },
    Control { cc: u8, value: u8, channel: u8 },
    ProgramChange { program: u8, channel: u8 },
    TrackName { name: String },
    Lyric { text: String },
    Marker { name: String },
    EndOfTrack,
}

// ------------------------------------------------------------------
// DMS 解析
// ------------------------------------------------------------------

const MAGIC: &[u8] = b"PortalSequenceData";
const MAGIC_LEN: usize = 18;

impl DmsDocument {
    fn parse(data: &[u8]) -> Result<Self, DmsError> {
        if data.len() < MAGIC_LEN + 4 {
            return Err(DmsError::InvalidDms);
        }
        if &data[0..MAGIC_LEN] != MAGIC {
            return Err(DmsError::InvalidDms);
        }
        let decompressed_len = u32::from_le_bytes([
            data[MAGIC_LEN],
            data[MAGIC_LEN + 1],
            data[MAGIC_LEN + 2],
            data[MAGIC_LEN + 3],
        ]) as usize;

        let compressed = &data[MAGIC_LEN + 4..];
        let mut decoder = flate2::read::ZlibDecoder::new(compressed);
        let mut raw = Vec::with_capacity(decompressed_len);
        decoder.read_to_end(&mut raw)?;

        if raw.len() != decompressed_len {
            return Err(DmsError::InvalidDms);
        }

        // C# DmsReader 会把整个数据包在一个虚拟的 type=0 wrapper 里
        let mut data_slice = raw.as_slice();
        let root = DmsNode {
            type_id: 0,
            computed_type: 0,
            children: {
                let mut children = Vec::new();
                while !data_slice.is_empty() {
                    children.push(parse_node(&mut data_slice, 0, 0)?);
                }
                children
            },
            data: Vec::new(),
        };

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
        for child in &node.children {
            if child.computed_type == NODE_TEMPO_VALUE {
                bpm = parse_float(&child.data)?;
            }
        }
        Some(RawMidiEvent {
            tick,
            kind: RawMidiEventKind::Tempo { bpm },
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
        // Domino 的 KeySig_Index 映射需要推断
        // 假设 index 范围是 -7..+7，对应 circle of fifths
        let sf = index.clamp(-7, 7) as i8;
        let mi = 0u8; // 假设大调
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
            if child.computed_type != NODE_ABS_TICK_POS && !child.children.is_empty() {
                // 未定义类型的子节点，假设第一个是 program number
                // 实际上对于 ProgramChangeEvent，子节点只有 AbsTickPos + 一个 data node
            }
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

    fn to_smf_bytes(&self) -> Result<Vec<u8>, DmsError> {
        use midly::num::{u4, u7, u15, u24, u28};
        use midly::{Format, Header, MetaMessage, MidiMessage, Timing, TrackEvent, TrackEventKind};

        let mut leaked: Vec<&'static [u8]> = Vec::new();
        let mut leak = |s: String| -> &'static [u8] {
            let b: &'static [u8] = Box::leak(s.into_bytes().into_boxed_slice());
            leaked.push(b);
            b
        };

        let mut smf_tracks: Vec<Vec<TrackEvent<'static>>> = Vec::new();

        for track in &self.tracks {
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
                    RawMidiEventKind::Tempo { bpm } => {
                        let mpq = (60_000_000.0 / *bpm).clamp(1.0, 16_777_215.0) as u32;
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
            timing: Timing::Metrical(u15::from_int_lossy(self.ppqn as u16)),
        };

        let mut buf = Vec::new();
        let track_iters: Vec<_> = smf_tracks.iter().map(|t| t.iter()).collect();
        midly::write_std(&header, track_iters.into_iter(), &mut buf)
            .map_err(|e| DmsError::MidiConvert(e.to_string()))?;

        Ok(buf)
    }
}

// ------------------------------------------------------------------
// DMS 树形节点解析
// ------------------------------------------------------------------

#[derive(Debug)]
struct DmsNode {
    type_id: u16,
    computed_type: u64,
    children: Vec<DmsNode>,
    data: Vec<u8>,
}

// 节点类型常量
const NODE_ROOT: u64 = 0x0000;
const NODE_SONG_NAME: u64 = 1000;
const NODE_SONG_COPYRIGHT: u64 = 1001;
const NODE_SONG_PPQN: u64 = 1002;
const NODE_TRACK: u64 = 1003;
const NODE_SONG_COMMENT: u64 = 1019;

const NODE_TRACK_CHANNEL: u64 = 1001 | (NODE_TRACK << 16);
const NODE_TRACK_NAME: u64 = 1002 | (NODE_TRACK << 16);
const NODE_TRACK_IS_DRUM: u64 = 1004 | (NODE_TRACK << 16);

const NODE_NOTE_EVENT: u64 = 2001 | (NODE_TRACK << 16);
const NODE_PROGRAM_CHANGE_EVENT: u64 = 2002 | (NODE_TRACK << 16);
const NODE_CONTROL_EVENT: u64 = 2003 | (NODE_TRACK << 16);
const NODE_CUSTOM_SYSEX_EVENT: u64 = 2004 | (NODE_TRACK << 16);
const NODE_COMMENT_EVENT: u64 = 2005 | (NODE_TRACK << 16);
const NODE_FORMULA_EVENT: u64 = 2007 | (NODE_TRACK << 16);
const NODE_TEMPO_EVENT: u64 = 2008 | (NODE_TRACK << 16);
const NODE_END_OF_TRACK_EVENT: u64 = 2009 | (NODE_TRACK << 16);
const NODE_LYRICS_EVENT: u64 = 2011 | (NODE_TRACK << 16);
const NODE_CUE_POINT_EVENT: u64 = 2012 | (NODE_TRACK << 16);
const NODE_MEASURE_LINK_EVENT: u64 = 2014 | (NODE_TRACK << 16);
const NODE_TIME_SIG_EVENT: u64 = 2015 | (NODE_TRACK << 16);
const NODE_KEY_SIG_EVENT: u64 = 2016 | (NODE_TRACK << 16);
const NODE_MARKER_EVENT: u64 = 2017 | (NODE_TRACK << 16);
const NODE_SCALE_EVENT: u64 = 2018 | (NODE_TRACK << 16);
const NODE_CHORD_EVENT: u64 = 2019 | (NODE_TRACK << 16);

// AbsTickPos 是特殊的：1001 | Track << 32
const NODE_ABS_TICK_POS: u64 = (1001u64) | ((1003u64) << 32);

const NODE_CURRENT_VARS: u64 = 1006;
const NODE_MIDI_OUT_CFG: u64 = 1008;
const NODE_KEY_PALETTE: u64 = 1017;
const NODE_PORT_CFG: u64 = 1018;
const NODE_TRACK_ONIONSKIN_DATA: u64 = 1010 | (NODE_TRACK << 16);

const PORT_CFG_A: u64 = 1000 | (NODE_PORT_CFG << 16);
const PORT_CFG_B: u64 = 1001 | (NODE_PORT_CFG << 16);
const PORT_CFG_C: u64 = 1002 | (NODE_PORT_CFG << 16);
const PORT_CFG_D: u64 = 1003 | (NODE_PORT_CFG << 16);
const PORT_CFG_E: u64 = 1004 | (NODE_PORT_CFG << 16);
const PORT_CFG_F: u64 = 1005 | (NODE_PORT_CFG << 16);
const PORT_CFG_G: u64 = 1006 | (NODE_PORT_CFG << 16);
const PORT_CFG_H: u64 = 1007 | (NODE_PORT_CFG << 16);
const PORT_CFG_I: u64 = 1008 | (NODE_PORT_CFG << 16);
const PORT_CFG_J: u64 = 1009 | (NODE_PORT_CFG << 16);
const PORT_CFG_K: u64 = 1010 | (NODE_PORT_CFG << 16);
const PORT_CFG_L: u64 = 1011 | (NODE_PORT_CFG << 16);
const PORT_CFG_M: u64 = 1012 | (NODE_PORT_CFG << 16);
const PORT_CFG_N: u64 = 1013 | (NODE_PORT_CFG << 16);
const PORT_CFG_O: u64 = 1014 | (NODE_PORT_CFG << 16);
const PORT_CFG_P: u64 = 1015 | (NODE_PORT_CFG << 16);

const NODE_NOTE_KEY_NUMBER: u64 = 2001 | (NODE_NOTE_EVENT << 16);
const NODE_NOTE_VELOCITY: u64 = 2002 | (NODE_NOTE_EVENT << 16);
const NODE_NOTE_GATE: u64 = 2003 | (NODE_NOTE_EVENT << 16);

const NODE_TEMPO_VALUE: u64 = 2001 | (NODE_TEMPO_EVENT << 16);

const NODE_TIME_SIG_NUMERATOR: u64 = 2001 | (NODE_TIME_SIG_EVENT << 16);
const NODE_TIME_SIG_DENOMINATOR: u64 = 2002 | (NODE_TIME_SIG_EVENT << 16);

const NODE_KEY_SIG_INDEX: u64 = 2001 | (NODE_KEY_SIG_EVENT << 16);

const NODE_CONTROL_TYPE: u64 = 2001 | (NODE_CONTROL_EVENT << 16);
const NODE_CONTROL_GATE: u64 = 2002 | (NODE_CONTROL_EVENT << 16);
const NODE_CONTROL_VALUE: u64 = 2003 | (NODE_CONTROL_EVENT << 16);

const NODE_LYRICS_LYRICS: u64 = 2001 | (NODE_LYRICS_EVENT << 16);
const NODE_MARKER_NAME: u64 = 2001 | (NODE_MARKER_EVENT << 16);

fn is_composite_node(node_type: u64) -> bool {
    matches!(
        node_type,
        NODE_ROOT
            | NODE_TRACK
            | NODE_NOTE_EVENT
            | NODE_PROGRAM_CHANGE_EVENT
            | NODE_CONTROL_EVENT
            | NODE_CUSTOM_SYSEX_EVENT
            | NODE_COMMENT_EVENT
            | NODE_FORMULA_EVENT
            | NODE_TEMPO_EVENT
            | NODE_END_OF_TRACK_EVENT
            | NODE_LYRICS_EVENT
            | NODE_CUE_POINT_EVENT
            | NODE_MEASURE_LINK_EVENT
            | NODE_TIME_SIG_EVENT
            | NODE_KEY_SIG_EVENT
            | NODE_MARKER_EVENT
            | NODE_SCALE_EVENT
            | NODE_CHORD_EVENT
            | NODE_CURRENT_VARS
            | NODE_MIDI_OUT_CFG
            | NODE_KEY_PALETTE
            | NODE_PORT_CFG
            | PORT_CFG_A
            | PORT_CFG_B
            | PORT_CFG_C
            | PORT_CFG_D
            | PORT_CFG_E
            | PORT_CFG_F
            | PORT_CFG_G
            | PORT_CFG_H
            | PORT_CFG_I
            | PORT_CFG_J
            | PORT_CFG_K
            | PORT_CFG_L
            | PORT_CFG_M
            | PORT_CFG_N
            | PORT_CFG_O
            | PORT_CFG_P
            | NODE_TRACK_ONIONSKIN_DATA
    )
}

fn compute_node_type(type_id: u16, _layer: i32, parent_type: u64) -> u64 {
    if parent_type == 0 {
        type_id as u64
    } else {
        let result = type_id as u64 | (parent_type << 16);
        if (result & 0x0000_0000_FFFF_0000) >= (2000u64 << 16)
            && (result & 0xFFFF_FFFF_0000_FFFF) == NODE_ABS_TICK_POS
        {
            NODE_ABS_TICK_POS
        } else {
            result
        }
    }
}

fn parse_node(data: &mut &[u8], layer: i32, parent_type: u64) -> Result<DmsNode, DmsError> {
    if data.len() < 2 {
        return Err(DmsError::InvalidDms);
    }
    let type_id = u16::from_le_bytes([data[0], data[1]]);
    *data = &data[2..];

    if data.len() < 4 {
        return Err(DmsError::InvalidDms);
    }
    let data_length = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    *data = &data[4..];

    let node_type = compute_node_type(type_id, layer, parent_type);

    if is_composite_node(node_type) {
        let mut children = Vec::new();
        let mut consumed = 0;
        while consumed < data_length {
            let before = data.len();
            let child = parse_node(data, layer + 1, node_type)?;
            consumed += before - data.len();
            children.push(child);
        }
        Ok(DmsNode {
            type_id,
            computed_type: node_type,
            children,
            data: Vec::new(),
        })
    } else {
        if data.len() < data_length {
            return Err(DmsError::InvalidDms);
        }
        let node_data = data[..data_length].to_vec();
        *data = &data[data_length..];
        Ok(DmsNode {
            type_id,
            computed_type: node_type,
            children: Vec::new(),
            data: node_data,
        })
    }
}

// ------------------------------------------------------------------
// 辅助解析函数
// ------------------------------------------------------------------

fn parse_integer(data: &[u8]) -> i64 {
    let mut result: i64 = 0;
    for (i, &b) in data.iter().enumerate() {
        result |= (b as i64) << (i * 8);
    }
    result
}

fn parse_float(data: &[u8]) -> Option<f64> {
    if data.len() >= 10 && u16::from_le_bytes([data[0], data[1]]) == 0 {
        let len = u32::from_le_bytes([data[2], data[3], data[4], data[5]]);
        if len == 4 && data.len() >= 10 {
            let val = f32::from_le_bytes([data[6], data[7], data[8], data[9]]);
            return Some(val as f64);
        } else if len == 8 && data.len() >= 14 {
            let val = f64::from_le_bytes([
                data[6], data[7], data[8], data[9], data[10], data[11], data[12], data[13],
            ]);
            return Some(val);
        }
    }
    None
}

fn parse_gbk_string(data: &[u8]) -> String {
    encoding_rs::GB18030.decode(data).0.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_song_dms() {
        let data = std::fs::read("../../assets/Song.dms").unwrap();
        let result = DmsFile::from_bytes(&data);
        if let Err(ref e) = result {
            eprintln!("Parse error: {}", e);
        }
        assert!(result.is_ok());
    }
}

