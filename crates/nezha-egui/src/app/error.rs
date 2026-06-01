use rust_i18n::t;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    MediaProbe(#[from] nezha_media::MediaError),

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    MidiLoad(String),

    #[error("{0}")]
    ArchiveOpen(String),

    #[error("{0}")]
    DmsLoad(String),

    #[error("{0}")]
    AudioRender(String),

    #[error("{0}")]
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

    /// 返回用户友好的错误消息（已翻译）
    pub fn user_message(&self) -> String {
        match self {
            AppError::MediaProbe(e) => format!("{}: {}", t!("error.media_probe"), e),
            AppError::Io(e) => format!("{}: {}", t!("error.io"), e),
            AppError::MidiLoad(e) => format!("{}: {}", t!("error.midi_load"), e),
            AppError::ArchiveOpen(e) => format!("{}: {}", t!("error.archive_open"), e),
            AppError::DmsLoad(e) => format!("{}: {}", t!("error.dms_load"), e),
            AppError::AudioRender(e) => format!("{}: {}", t!("error.audio_render"), e),
            AppError::Export(e) => format!("{}: {}", t!("error.export"), e),
            AppError::Other(e) => e.clone(),
        }
    }
}
