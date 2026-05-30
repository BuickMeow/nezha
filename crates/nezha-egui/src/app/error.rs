use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("媒体探测失败: {0}")]
    MediaProbe(#[from] nezha_media::MediaError),

    #[error("图片加载失败: {0}")]
    ImageLoad(#[from] image::ImageError),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("MIDI 加载失败: {0}")]
    MidiLoad(String),

    #[error("压缩包打开失败: {0}")]
    ArchiveOpen(String),

    #[error("读取压缩包内文件失败: {0}")]
    ArchiveRead(String),

    #[error("音频渲染失败: {0}")]
    AudioRender(String),

    #[error("导出失败: {0}")]
    Export(String),

    #[error("{0}")]
    Other(String),
}

impl AppError {
    pub fn midi_load(e: impl std::fmt::Display) -> Self {
        Self::MidiLoad(e.to_string())
    }

    pub fn archive_open(e: impl std::fmt::Display) -> Self {
        Self::ArchiveOpen(e.to_string())
    }

    pub fn archive_read(e: impl std::fmt::Display) -> Self {
        Self::ArchiveRead(e.to_string())
    }

    pub fn audio_render(e: impl std::fmt::Display) -> Self {
        Self::AudioRender(e.to_string())
    }

    pub fn export(e: impl std::fmt::Display) -> Self {
        Self::Export(e.to_string())
    }
}
