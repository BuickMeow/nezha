mod audio_decoder;
mod ffmpeg_util;
mod image_loader;
mod metadata;
mod video_decoder;

pub use audio_decoder::{AudioDecoder, DecodedAudio};
pub use ffmpeg_util::ffmpeg_path;
pub use image_loader::load_image;
pub use metadata::{MediaInfo, MediaType, probe_media};
pub use video_decoder::{DecodedFrame, StreamingDecoder};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("ffmpeg not found")]
    FfmpegNotFound,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg probe failed: {0}")]
    ProbeFailed(String),
    #[error("ffmpeg decode failed: {0}")]
    DecodeFailed(String),
    #[error("image decode error: {0}")]
    ImageError(#[from] image::ImageError),
    #[error("unsupported media type")]
    UnsupportedType,
    #[error("invalid frame data: {0}")]
    InvalidFrame(String),
}
