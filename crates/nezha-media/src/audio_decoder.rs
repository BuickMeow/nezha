use std::process::{Command, Stdio};

use crate::MediaError;
use crate::ffmpeg_util::ffmpeg_path;

pub struct DecodedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
    pub duration_secs: f64,
}

pub struct AudioDecoder;

impl AudioDecoder {
    pub fn decode_file(path: &str, target_sample_rate: u32) -> Result<DecodedAudio, MediaError> {
        let ffmpeg = ffmpeg_path()?;

        let output = Command::new(&ffmpeg)
            .args([
                "-v",
                "quiet",
                "-i",
                path,
                "-ar",
                &target_sample_rate.to_string(),
                "-ac",
                "2",
                "-f",
                "f32le",
                "-acodec",
                "pcm_f32le",
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

        let samples = bytes_to_f32_vec(&output.stdout);
        let channels = 2u16;
        let frames = samples.len() / channels as usize;
        let duration_secs = frames as f64 / target_sample_rate as f64;

        Ok(DecodedAudio {
            sample_rate: target_sample_rate,
            channels,
            samples,
            duration_secs,
        })
    }
}

fn bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    let mut samples = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let s = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        samples.push(s);
    }
    samples
}
