use std::process::{Command, Stdio};

use crate::MediaError;
use crate::ffmpeg_util::ffmpeg_path;

pub struct DecodedFrame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub struct VideoDecoder {
    path: String,
    width: u32,
    height: u32,
}

impl VideoDecoder {
    pub fn new(path: &str, width: u32, height: u32) -> Self {
        Self {
            path: path.to_string(),
            width,
            height,
        }
    }

    pub fn decode_frame(&self, timestamp_secs: f64) -> Result<DecodedFrame, MediaError> {
        let ffmpeg = ffmpeg_path()?;

        let output = Command::new(&ffmpeg)
            .args([
                "-v",
                "quiet",
                "-ss",
                &format!("{:.4}", timestamp_secs),
                "-i",
                &self.path,
                "-vframes",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .map_err(|_| MediaError::FfmpegNotFound)?;

        if !output.status.success() {
            return Err(MediaError::DecodeFailed(format!(
                "ffmpeg exited with code {:?}",
                output.status.code()
            )));
        }

        let expected_size = (self.width * self.height * 4) as usize;
        if output.stdout.len() != expected_size {
            return Err(MediaError::InvalidFrame(format!(
                "expected {} bytes, got {}",
                expected_size,
                output.stdout.len()
            )));
        }

        Ok(DecodedFrame {
            width: self.width,
            height: self.height,
            rgba: output.stdout,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}
