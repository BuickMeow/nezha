use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("媒体探测失败: {0}")]
    MediaProbe(#[from] nezha_media::MediaError),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("MIDI 加载失败: {0}")]
    MidiLoad(String),

    #[error("压缩包打开失败: {0}")]
    ArchiveOpen(String),

    #[error("DMS 解析失败: {0}")]
    DmsLoad(String),

    #[error("音频渲染失败: {0}")]
    AudioRender(String),

    #[error("导出失败: {0}")]
    Export(String),

    #[error("{0}")]
    Other(String),
}

impl From<nezha_core::MidiError> for AppError {
    fn from(e: nezha_core::MidiError) -> Self {
        Self::MidiLoad(e.to_string())
    }
}

impl From<nezha_archive::ArchiveError> for AppError {
    fn from(e: nezha_archive::ArchiveError) -> Self {
        Self::ArchiveOpen(e.to_string())
    }
}

impl From<nezha_dms::DmsError> for AppError {
    fn from(e: nezha_dms::DmsError) -> Self {
        Self::DmsLoad(e.to_string())
    }
}

impl From<nezha_encoder::EncoderError> for AppError {
    fn from(e: nezha_encoder::EncoderError) -> Self {
        Self::Export(e.to_string())
    }
}

impl From<nezha_xsynth::RenderError> for AppError {
    fn from(e: nezha_xsynth::RenderError) -> Self {
        Self::AudioRender(e.to_string())
    }
}

impl AppError {
    pub fn midi_load(e: impl std::fmt::Display) -> Self {
        Self::MidiLoad(e.to_string())
    }

    pub fn archive_open(e: impl std::fmt::Display) -> Self {
        Self::ArchiveOpen(e.to_string())
    }

    pub fn archive_read(e: impl std::fmt::Display) -> Self {
        Self::ArchiveOpen(e.to_string())
    }

    pub fn audio_render(e: impl std::fmt::Display) -> Self {
        Self::AudioRender(e.to_string())
    }

    pub fn export(e: impl std::fmt::Display) -> Self {
        Self::Export(e.to_string())
    }
}
