use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::{ExportConfig, VideoCodec};

#[derive(Debug, thiserror::Error)]
pub enum EncoderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg exited with code {0:?}")]
    FfmpegFailed(Option<i32>),
    #[error("ffmpeg not found")]
    FfmpegNotFound,
    #[error("WAV error: {0}")]
    Wav(#[from] hound::Error),
}

#[derive(Debug)]
pub struct FfmpegEncoder {
    process: std::process::Child,
    sender: Option<std::sync::mpsc::SyncSender<Vec<u8>>>,
    join_handle: Option<std::thread::JoinHandle<Result<(), EncoderError>>>,
    /// Temporary WAV file path to clean up on drop.
    temp_wav: Option<PathBuf>,
}

impl FfmpegEncoder {
    pub fn new(config: &ExportConfig) -> Result<Self, EncoderError> {
        let ffmpeg = ffmpeg_path();

        // Check for bundled ffmpeg
        let is_bundled_path = ffmpeg
            .file_name()
            .is_some_and(|n| n == "ffmpeg" || n == "ffmpeg.exe")
            && ffmpeg.parent().is_some_and(|p| p != "");
        if is_bundled_path && !ffmpeg.exists() {
            return Err(EncoderError::FfmpegNotFound);
        }

        // Write audio PCM to a temp WAV file if present
        let temp_wav: Option<PathBuf> = if let Some(pcm) = &config.audio_pcm {
            let tmp = std::env::temp_dir().join(format!("nezha_audio_{}.wav", std::process::id()));
            write_wav_file(&tmp, pcm, config.audio_sample_rate, config.audio_channels)?;
            Some(tmp)
        } else {
            None
        };

        let args = build_ffmpeg_args(config, temp_wav.as_deref());

        let mut process = Command::new(&ffmpeg)
            .args(&args)
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdin = process.stdin.take().expect("ffmpeg stdin piped");
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(64);

        let join_handle = std::thread::spawn(move || {
            for frame_data in rx {
                stdin.write_all(&frame_data)?;
            }
            Ok(())
        });

        Ok(Self {
            process,
            sender: Some(tx),
            join_handle: Some(join_handle),
            temp_wav,
        })
    }

    pub fn write_frame(&mut self, frame_data: Vec<u8>) -> Result<(), EncoderError> {
        if let Some(sender) = &self.sender {
            sender
                .send(frame_data)
                .map_err(|_| EncoderError::FfmpegFailed(None))?;
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), EncoderError> {
        // Close sender so background thread finishes
        self.sender.take();

        // Wait for background thread
        if let Some(handle) = self.join_handle.take() {
            handle
                .join()
                .map_err(|_| EncoderError::FfmpegFailed(None))??;
        }

        // Wait for ffmpeg process
        let status = self.process.wait()?;
        if !status.success() {
            let stderr = self
                .process
                .stderr
                .as_mut()
                .and_then(|s| {
                    use std::io::Read;
                    let mut buf = String::new();
                    s.read_to_string(&mut buf).ok()?;
                    Some(buf)
                })
                .unwrap_or_default();
            if !stderr.is_empty() {
                eprintln!("ffmpeg stderr:\n{}", stderr);
            }
            return Err(EncoderError::FfmpegFailed(status.code()));
        }

        // Clean up temp WAV
        if let Some(tmp) = &self.temp_wav {
            let _ = std::fs::remove_file(tmp);
        }

        Ok(())
    }
}

impl Drop for FfmpegEncoder {
    fn drop(&mut self) {
        // Kill ffmpeg if user cancels
        let _ = self.process.kill();
        let _ = self.process.wait();

        // Wait for background thread
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }

        // Clean up temp WAV
        if let Some(tmp) = &self.temp_wav {
            let _ = std::fs::remove_file(tmp);
        }
    }
}

/// Write PCM samples to a WAV file.
fn write_wav_file(
    path: &Path,
    pcm: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<(), EncoderError> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &sample in pcm {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;
    Ok(())
}

/// Return the ffmpeg executable path.
pub fn ffmpeg_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let name = if cfg!(target_os = "windows") {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        };
        let bundled = dir.join(name);
        if bundled.exists() {
            return bundled;
        }
    }

    if cfg!(target_os = "windows") {
        PathBuf::from("ffmpeg.exe")
    } else {
        PathBuf::from("ffmpeg")
    }
}

fn build_ffmpeg_args(config: &ExportConfig, audio_wav: Option<&Path>) -> Vec<String> {
    let mut args = Vec::new();

    // Audio input first (if present)
    if let Some(wav_path) = audio_wav {
        args.push("-i".to_string());
        args.push(wav_path.to_string_lossy().to_string());
    }

    // Video input: rawvideo from stdin
    args.push("-f".to_string());
    args.push("rawvideo".to_string());
    args.push("-pix_fmt".to_string());
    args.push("bgra".to_string());
    args.push("-s".to_string());
    args.push(format!("{}x{}", config.width, config.height));
    args.push("-r".to_string());
    args.push(format!("{:.3}", config.fps));
    args.push("-thread_queue_size".to_string());
    args.push("512".to_string());
    args.push("-i".to_string());
    args.push("-".to_string());

    // Map video from the second input (index 1)
    args.push("-map".to_string());
    args.push("1:v".to_string());

    // Map audio from the first input (index 0) if present
    if audio_wav.is_some() {
        args.push("-map".to_string());
        args.push("0:a".to_string());
        args.push("-shortest".to_string());
    }

    // Video encoder
    args.push("-c:v".to_string());
    args.push(config.codec.ffmpeg_encoder().to_string());

    // Quality settings
    match &config.codec {
        VideoCodec::H264 | VideoCodec::H265 => {
            args.push("-crf".to_string());
            args.push(config.quality.crf().to_string());
            args.push("-preset".to_string());
            args.push(config.quality.preset().to_string());
            args.push("-pix_fmt".to_string());
            args.push("yuv420p".to_string());
        }
        VideoCodec::Vp9 => {
            args.push("-crf".to_string());
            args.push(config.quality.crf().to_string());
            args.push("-b:v".to_string());
            args.push("0".to_string());
            args.push("-pix_fmt".to_string());
            args.push("yuv420p".to_string());
        }
        VideoCodec::Av1 => {
            args.push("-crf".to_string());
            args.push(config.quality.crf().to_string());
            args.push("-pix_fmt".to_string());
            args.push("yuv420p".to_string());
        }
        VideoCodec::ProRes => {
            args.push("-profile:v".to_string());
            args.push("3".to_string());
            args.push("-pix_fmt".to_string());
            args.push("yuv422p".to_string());
            args.push("-qscale:v".to_string());
            args.push("9".to_string());
        }
    }

    // Audio encoder (copy from source, or re-encode to AAC)
    if audio_wav.is_some() {
        args.push("-c:a".to_string());
        args.push("aac".to_string());
        args.push("-b:a".to_string());
        args.push("192k".to_string());
    }

    // Container format
    args.push("-f".to_string());
    args.push(config.container.ffmpeg_muxer().to_string());

    // Overwrite output
    args.push("-y".to_string());

    // Output path
    args.push(config.output_path.to_string_lossy().to_string());

    args
}
