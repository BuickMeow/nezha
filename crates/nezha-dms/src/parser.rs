use std::io::Read;

use crate::DmsError;

// ------------------------------------------------------------------
// 文件读取
// ------------------------------------------------------------------

const MAGIC: &[u8] = b"PortalSequenceData";
pub const MAGIC_LEN: usize = 18;

/// 读取 DMS 文件头，ZLib 解压，返回原始树形数据。
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, DmsError> {
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

    Ok(raw)
}

// ------------------------------------------------------------------
// DMS 树形节点
// ------------------------------------------------------------------

#[derive(Debug)]
#[allow(dead_code)]
pub struct DmsNode {
    pub type_id: u16,
    pub computed_type: u64,
    pub children: Vec<DmsNode>,
    pub data: Vec<u8>,
}

/// C# DmsReader 会把整个数据包在一个虚拟的 type=0 wrapper 里，
/// 然后它的 children 才是文件中的实际节点。
pub fn parse_root(data: &[u8]) -> Result<DmsNode, DmsError> {
    let mut slice = data;
    let mut children = Vec::new();
    while !slice.is_empty() {
        children.push(parse_node(&mut slice, 0, 0)?);
    }
    Ok(DmsNode {
        type_id: 0,
        computed_type: 0,
        children,
        data: Vec::new(),
    })
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
// 节点类型常量
// ------------------------------------------------------------------

pub const NODE_ROOT: u64 = 0x0000;
pub const NODE_SONG_PPQN: u64 = 1002;
pub const NODE_TRACK: u64 = 1003;

pub const NODE_TRACK_CHANNEL: u64 = 1001 | (NODE_TRACK << 16);
pub const NODE_TRACK_NAME: u64 = 1002 | (NODE_TRACK << 16);

pub const NODE_NOTE_EVENT: u64 = 2001 | (NODE_TRACK << 16);
pub const NODE_PROGRAM_CHANGE_EVENT: u64 = 2002 | (NODE_TRACK << 16);
pub const NODE_CONTROL_EVENT: u64 = 2003 | (NODE_TRACK << 16);
pub const NODE_TEMPO_EVENT: u64 = 2008 | (NODE_TRACK << 16);
pub const NODE_END_OF_TRACK_EVENT: u64 = 2009 | (NODE_TRACK << 16);
pub const NODE_LYRICS_EVENT: u64 = 2011 | (NODE_TRACK << 16);
pub const NODE_TIME_SIG_EVENT: u64 = 2015 | (NODE_TRACK << 16);
pub const NODE_KEY_SIG_EVENT: u64 = 2016 | (NODE_TRACK << 16);
pub const NODE_MARKER_EVENT: u64 = 2017 | (NODE_TRACK << 16);

pub const NODE_ABS_TICK_POS: u64 = (1001u64) | ((1003u64) << 32);

pub const NODE_NOTE_KEY_NUMBER: u64 = 2001 | (NODE_NOTE_EVENT << 16);
pub const NODE_NOTE_VELOCITY: u64 = 2002 | (NODE_NOTE_EVENT << 16);
pub const NODE_NOTE_GATE: u64 = 2003 | (NODE_NOTE_EVENT << 16);

pub const NODE_TEMPO_VALUE: u64 = 2001 | (NODE_TEMPO_EVENT << 16);
pub const NODE_TEMPO_BASE_GATE: u64 = 2002 | (NODE_TEMPO_EVENT << 16);

pub const NODE_TIME_SIG_NUMERATOR: u64 = 2001 | (NODE_TIME_SIG_EVENT << 16);
pub const NODE_TIME_SIG_DENOMINATOR: u64 = 2002 | (NODE_TIME_SIG_EVENT << 16);

pub const NODE_KEY_SIG_INDEX: u64 = 2001 | (NODE_KEY_SIG_EVENT << 16);

pub const NODE_CONTROL_TYPE: u64 = 2001 | (NODE_CONTROL_EVENT << 16);
pub const NODE_CONTROL_VALUE: u64 = 2003 | (NODE_CONTROL_EVENT << 16);

pub const NODE_LYRICS_LYRICS: u64 = 2001 | (NODE_LYRICS_EVENT << 16);
pub const NODE_MARKER_NAME: u64 = 2001 | (NODE_MARKER_EVENT << 16);

// Composite node 判断
fn is_composite_node(node_type: u64) -> bool {
    matches!(
        node_type,
        NODE_ROOT
            | NODE_TRACK
            | NODE_NOTE_EVENT
            | NODE_PROGRAM_CHANGE_EVENT
            | NODE_CONTROL_EVENT
            | NODE_TEMPO_EVENT
            | NODE_END_OF_TRACK_EVENT
            | NODE_LYRICS_EVENT
            | NODE_TIME_SIG_EVENT
            | NODE_KEY_SIG_EVENT
            | NODE_MARKER_EVENT
            | 1006 // CurrentVars
            | 1008 // MidiOutCfg
            | 1017 // KeyPalette
            | 1018 // PortCfg
            | 6684672 // 1000 | (1018 << 16)
            | 6686721 // 1001 | (1018 << 16)
            | 6688770 // 1002 | (1018 << 16)
            | 6690819 // 1003 | (1018 << 16)
            | 6692868 // 1004 | (1018 << 16)
            | 6694917 // 1005 | (1018 << 16)
            | 6696966 // 1006 | (1018 << 16)
            | 6699015 // 1007 | (1018 << 16)
            | 6701064 // 1008 | (1018 << 16)
            | 6703113 // 1009 | (1018 << 16)
            | 6705162 // 1010 | (1018 << 16)
            | 6707211 // 1011 | (1018 << 16)
            | 6709260 // 1012 | (1018 << 16)
            | 6711309 // 1013 | (1018 << 16)
            | 6713358 // 1014 | (1018 << 16)
            | 6715407 // 1015 | (1018 << 16)
            | 6574090 // 1010 | (1003 << 16) Track_OnionskinData
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

// ------------------------------------------------------------------
// 辅助解析函数
// ------------------------------------------------------------------

pub fn parse_integer(data: &[u8]) -> i64 {
    let mut result: i64 = 0;
    let n = data.len().min(8);
    for (i, &b) in data[..n].iter().enumerate() {
        result |= (b as i64) << (i * 8);
    }
    result
}

pub fn parse_float(data: &[u8]) -> Option<f64> {
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

pub fn parse_gbk_string(data: &[u8]) -> String {
    encoding_rs::GB18030.decode(data).0.into_owned()
}
