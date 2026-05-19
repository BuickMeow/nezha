use std::io;
use std::path::Path;

mod converter;
mod model;
mod parser;

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
// 进度回调
// ------------------------------------------------------------------

/// DMS 加载过程中的进度事件。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DmsLoadProgress {
    /// 正在解压 ZLib 数据。
    Decompressing,
    /// 正在解析树形结构。
    ParsingTree,
    /// 正在提取 MIDI 事件（第 N 个音轨 / 总共 M 个）。
    ExtractingEvents {
        current_track: usize,
        total_tracks: usize,
    },
    /// 正在生成 SMF 字节流。
    GeneratingSmf,
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
        Self::from_bytes_with_progress(data, |_| {})
    }

    /// 带进度回调的版本。
    pub fn from_bytes_with_progress(
        data: &[u8],
        mut progress: impl FnMut(DmsLoadProgress),
    ) -> Result<nezha_core::MidiFile, DmsError> {
        progress(DmsLoadProgress::Decompressing);
        let doc = model::DmsDocument::parse(data)?;

        progress(DmsLoadProgress::ExtractingEvents {
            current_track: 0,
            total_tracks: doc.tracks.len(),
        });
        for (i, _) in doc.tracks.iter().enumerate() {
            progress(DmsLoadProgress::ExtractingEvents {
                current_track: i + 1,
                total_tracks: doc.tracks.len(),
            });
        }

        progress(DmsLoadProgress::GeneratingSmf);
        let midi_bytes = converter::to_smf_bytes(&doc)?;
        nezha_core::MidiFile::load_from_bytes(&midi_bytes)
            .map_err(|e| DmsError::MidiConvert(e.to_string()))
    }
}

// ------------------------------------------------------------------
// 测试
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_data() -> Option<Vec<u8>> {
        std::fs::read("../../assets/Song.dms").ok()
    }

    #[test]
    fn test_parse_song_dms() {
        let Some(data) = load_test_data() else {
            eprintln!("Skipping test: ../../assets/Song.dms not found");
            return;
        };
        let result = DmsFile::from_bytes(&data);
        if let Err(ref e) = result {
            eprintln!("Parse error: {}", e);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_song_dms_with_progress() {
        let Some(data) = load_test_data() else {
            eprintln!("Skipping test: ../../assets/Song.dms not found");
            return;
        };
        let mut events = Vec::new();
        let result = DmsFile::from_bytes_with_progress(&data, |p| events.push(p));
        assert!(result.is_ok());
        assert!(
            events.contains(&DmsLoadProgress::Decompressing),
            "should have Decompressing"
        );
        assert!(
            events.contains(&DmsLoadProgress::GeneratingSmf),
            "should have GeneratingSmf"
        );
    }
}
