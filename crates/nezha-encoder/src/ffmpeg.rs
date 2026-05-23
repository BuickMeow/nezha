use std::io::{BufRead, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

use crate::config::{EncoderBackend, ExportConfig, QualityPreset, VideoCodec};

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
    #[error("frame data unexpected size: got {got}, expected {expected} bytes")]
    FrameSizeMismatch { got: usize, expected: usize },
}

#[derive(Debug)]
pub struct FfmpegEncoder {
    process: std::process::Child,
    sender: Option<crossbeam_channel::Sender<Vec<u8>>>,
    join_handle: Option<std::thread::JoinHandle<Result<(), EncoderError>>>,
    /// Temporary WAV file path to clean up on drop.
    temp_wav: Option<PathBuf>,
    /// Frame dimensions for BGRA→YUV420p conversion.
    width: u32,
    height: u32,
}

impl FfmpegEncoder {
    pub fn new(config: &ExportConfig) -> Result<Self, EncoderError> {
        let ffmpeg = ffmpeg_path()?;

        tracing::info!(path = %ffmpeg.display(), "Starting ffmpeg encoder");

        // Write audio PCM to a temp WAV file if present
        let temp_wav: Option<PathBuf> = if let Some(pcm) = &config.audio_pcm {
            let tmp = std::env::temp_dir().join(format!("nezha_audio_{}.wav", std::process::id()));
            write_wav_file(&tmp, pcm, config.audio_sample_rate, config.audio_channels)?;
            Some(tmp)
        } else {
            None
        };

        let args = build_ffmpeg_args(config, temp_wav.as_deref());

        tracing::debug!(?args, "ffmpeg arguments");

        let mut process = Command::new(&ffmpeg)
            .args(&args)
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // ── stderr logger thread: read ffmpeg stderr in real-time ──
        let stderr = process.stderr.take().expect("ffmpeg stderr piped");
        thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim_end();
                        if !trimmed.is_empty() {
                            tracing::warn!("[ffmpeg] {}", trimmed);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("[ffmpeg] stderr read error: {}", e);
                        break;
                    }
                }
            }
        });

        let mut stdin = BufWriter::with_capacity(
            1024 * 1024, // 1 MB buffer to batch writes
            process.stdin.take().expect("ffmpeg stdin piped"),
        );
        let (tx, rx) = crossbeam_channel::bounded::<Vec<u8>>(256);

        let join_handle = std::thread::spawn(move || {
            for frame_data in rx {
                stdin.write_all(&frame_data)?;
            }
            // Flush remaining data so ffmpeg sees everything
            stdin.flush()?;
            // Drop stdin to signal EOF to ffmpeg
            drop(stdin);
            Ok(())
        });

        Ok(Self {
            process,
            sender: Some(tx),
            join_handle: Some(join_handle),
            temp_wav,
            width: config.width,
            height: config.height,
        })
    }

    /// Write a BGRA frame (width×height×4 bytes) to the encoder.
    ///
    /// The frame is converted from BGRA to YUV420p internally before being
    /// sent to ffmpeg. This reduces pipe bandwidth by ~62% and skips the
    /// costly pixel-format conversion inside ffmpeg.
    pub fn write_frame(&mut self, frame_data: Vec<u8>) -> Result<(), EncoderError> {
        let expected = (self.width * self.height * 4) as usize;
        if frame_data.len() != expected {
            return Err(EncoderError::FrameSizeMismatch {
                got: frame_data.len(),
                expected,
            });
        }

        let yuv = bgra_to_yuv420p(&frame_data, self.width, self.height);

        if let Some(sender) = &self.sender {
            sender
                .send(yuv)
                .map_err(|_| EncoderError::FfmpegFailed(None))?;
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), EncoderError> {
        // Close sender so background thread finishes
        self.sender.take();

        // Wait for background thread (which flushes & drops stdin)
        if let Some(handle) = self.join_handle.take() {
            handle
                .join()
                .map_err(|_| EncoderError::FfmpegFailed(None))??;
        }

        // Wait for ffmpeg process
        let status = self.process.wait()?;
        if !status.success() {
            tracing::error!(
                code = status.code(),
                "ffmpeg exited with non-zero status"
            );
            return Err(EncoderError::FfmpegFailed(status.code()));
        }

        tracing::info!("ffmpeg encoding completed successfully");

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

// ---------------------------------------------------------------------------
// BGRA → YUV420p conversion
// ---------------------------------------------------------------------------

/// Convert a BGRA frame (4 bytes per pixel) to YUV420p (1.5 bytes per pixel).
///
/// Uses integer arithmetic based on ITU-R BT.601 full-range coefficients.
/// YUV420p layout: Y plane (W×H), then U plane (W/2 × H/2), then V plane (W/2 × H/2).
fn bgra_to_yuv420p(bgra: &[u8], width: u32, height: u32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = (w / 2) * (h / 2);
    let mut yuv = vec![0u8; y_size + uv_size * 2];

    let (y_plane, rest) = yuv.split_at_mut(y_size);
    let (u_plane, v_plane) = rest.split_at_mut(uv_size);

    // Y plane — full resolution
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) * 4;
            let b = bgra[idx] as i32;
            let g = bgra[idx + 1] as i32;
            let r = bgra[idx + 2] as i32;
            let y_val = (77 * r + 150 * g + 29 * b) >> 8;
            y_plane[y * w + x] = y_val.clamp(0, 255) as u8;
        }
    }

    // U/V planes — 2×2 subsampled
    for y in (0..h).step_by(2) {
        for x in (0..w).step_by(2) {
            let mut u_acc: i64 = 0;
            let mut v_acc: i64 = 0;

            for dy in 0..2 {
                for dx in 0..2 {
                    let px = x + dx;
                    let py = y + dy;
                    if px < w && py < h {
                        let idx = (py * w + px) * 4;
                        let b = bgra[idx] as i32;
                        let g = bgra[idx + 1] as i32;
                        let r = bgra[idx + 2] as i32;

                        u_acc += (-43 * r - 85 * g + 128 * b) as i64;
                        v_acc += (128 * r - 107 * g - 21 * b) as i64;
                    }
                }
            }

            let uv_idx = (y / 2) * (w / 2) + (x / 2);
            let u_val = ((u_acc / 4) >> 8) + 128;
            let v_val = ((v_acc / 4) >> 8) + 128;
            u_plane[uv_idx] = u_val.clamp(0, 255) as u8;
            v_plane[uv_idx] = v_val.clamp(0, 255) as u8;
        }
    }

    yuv
}

// ---------------------------------------------------------------------------
// WAV helper
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// ffmpeg discovery
// ---------------------------------------------------------------------------

/// Return the ffmpeg executable path.
///
/// Checks (in order):
///   1. Bundled ffmpeg next to the executable.
///   2. `ffmpeg` / `ffmpeg.exe` available via `PATH`.
pub fn ffmpeg_path() -> Result<PathBuf, EncoderError> {
    let exe_name = if cfg!(target_os = "windows") {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };

    // 1. Bundled next to the executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join(exe_name);
            if bundled.is_file() {
                return Ok(bundled);
            }
        }
    }

    // 2. Available via PATH
    if Command::new(exe_name)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
    {
        return Ok(PathBuf::from(exe_name));
    }

    Err(EncoderError::FfmpegNotFound)
}

// ---------------------------------------------------------------------------
// ffmpeg argument builder
// ---------------------------------------------------------------------------

fn build_ffmpeg_args(config: &ExportConfig, audio_wav: Option<&Path>) -> Vec<String> {
    let mut args = Vec::new();

    // Audio input first (if present)
    if let Some(wav_path) = audio_wav {
        args.push("-i".to_string());
        args.push(wav_path.to_string_lossy().to_string());
    }

    // ── Video input: raw YUV420p from stdin ──
    args.push("-f".to_string());
    args.push("rawvideo".to_string());
    args.push("-pix_fmt".to_string());
    args.push("yuv420p".to_string());
    args.push("-s".to_string());
    args.push(format!("{}x{}", config.width, config.height));
    args.push("-r".to_string());
    args.push(format!("{:.3}", config.fps));
    args.push("-thread_queue_size".to_string());
    args.push("2048".to_string());
    args.push("-i".to_string());
    args.push("-".to_string());

    // ── Map streams ──
    //   When audio is present: input 0 = WAV, input 1 = stdin video
    //   When audio is absent:  input 0 = stdin video
    let video_input_idx = if audio_wav.is_some() { 1 } else { 0 };

    args.push("-map".to_string());
    args.push(format!("{}:v", video_input_idx));

    if audio_wav.is_some() {
        args.push("-map".to_string());
        args.push("0:a".to_string());
        args.push("-shortest".to_string());
    }

    // ── Multi-threading ──
    args.push("-threads".to_string());
    args.push("0".to_string());

    // ── Video encoder ──
    args.push("-c:v".to_string());
    args.push(config.ffmpeg_encoder_name());

    // ── Quality / codec-specific settings ──
    match &config.backend {
        EncoderBackend::Software => build_software_args(&mut args, &config.codec, &config.quality),
        EncoderBackend::VideoToolbox => {
            build_videotoolbox_args(&mut args, &config.codec, &config.quality)
        }
        EncoderBackend::Nvenc => build_nvenc_args(&mut args, &config.codec, &config.quality),
        EncoderBackend::Amf => build_amf_args(&mut args, &config.codec, &config.quality),
        EncoderBackend::Qsv => build_qsv_args(&mut args, &config.codec, &config.quality),
        EncoderBackend::Vaapi => build_vaapi_args(&mut args, &config.codec, &config.quality),
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

// ---------------------------------------------------------------------------
// Backend-specific encoder quality arguments
// ---------------------------------------------------------------------------

/// Software encoders: libx264 / libx265 / prores_ks / libvpx-vp9 / libsvtav1.
fn build_software_args(args: &mut Vec<String>, codec: &VideoCodec, quality: &QualityPreset) {
    match codec {
        VideoCodec::H264 | VideoCodec::H265 => {
            args.push("-crf".to_string());
            args.push(quality.crf().to_string());
            args.push("-preset".to_string());
            args.push(quality.preset().to_string());
            args.push("-pix_fmt".to_string());
            args.push("yuv420p".to_string());
        }
        VideoCodec::Vp9 => {
            args.push("-crf".to_string());
            args.push(quality.crf().to_string());
            args.push("-b:v".to_string());
            args.push("0".to_string());
            args.push("-pix_fmt".to_string());
            args.push("yuv420p".to_string());
            // VP9 threading
            args.push("-row-mt".to_string());
            args.push("1".to_string());
            args.push("-tile-columns".to_string());
            args.push("2".to_string());
        }
        VideoCodec::Av1 => {
            args.push("-crf".to_string());
            args.push(quality.crf().to_string());
            args.push("-pix_fmt".to_string());
            args.push("yuv420p".to_string());
            // SVT-AV1 threading
            args.push("-svtav1-params".to_string());
            args.push(format!("lp={}", num_cpus()));
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
}

/// macOS VideoToolbox: h264_videotoolbox / hevc_videotoolbox / prores_videotoolbox.
///
/// These use target bitrate and a quality level (1 = best, 4 = fastest) instead of CRF.
fn build_videotoolbox_args(args: &mut Vec<String>, codec: &VideoCodec, quality: &QualityPreset) {
    match codec {
        VideoCodec::H264 | VideoCodec::H265 => {
            let (bitrate, vt_q) = match quality {
                QualityPreset::High => ("50M", "1"),
                QualityPreset::Medium => ("20M", "2"),
                QualityPreset::Low => ("10M", "4"),
            };
            args.push("-b:v".to_string());
            args.push(bitrate.to_string());
            args.push("-quality".to_string());
            args.push(vt_q.to_string());
            args.push("-pix_fmt".to_string());
            args.push("yuv420p".to_string());
        }
        VideoCodec::ProRes => {
            let bitrate = match quality {
                QualityPreset::High => "100M",
                QualityPreset::Medium => "50M",
                QualityPreset::Low => "20M",
            };
            args.push("-b:v".to_string());
            args.push(bitrate.to_string());
            args.push("-pix_fmt".to_string());
            args.push("yuv422p".to_string());
        }
        _ => {
            // Unsupported codec/backend combo — ffmpeg will error, but we can try.
            // Just set a generic bitrate.
            args.push("-b:v".to_string());
            args.push("20M".to_string());
        }
    }
}

/// NVIDIA NVENC: h264_nvenc / hevc_nvenc / av1_nvenc (Windows & Linux).
///
/// Uses constant-quality (`-cq`) with VBR rate control, plus a preset (p1–p7).
fn build_nvenc_args(args: &mut Vec<String>, codec: &VideoCodec, quality: &QualityPreset) {
    let cq = match quality {
        QualityPreset::High => "18",
        QualityPreset::Medium => "23",
        QualityPreset::Low => "28",
    };
    let preset = match quality {
        // p1 = fastest, p7 = slowest
        QualityPreset::High => "p5",
        QualityPreset::Medium => "p4",
        QualityPreset::Low => "p2",
    };

    args.push("-cq".to_string());
    args.push(cq.to_string());
    args.push("-rc".to_string());
    args.push("vbr".to_string());
    args.push("-preset".to_string());
    args.push(preset.to_string());
    args.push("-pix_fmt".to_string());
    args.push(codec.ffmpeg_pix_fmt().to_string());
}

/// AMD AMF: h264_amf / hevc_amf / av1_amf (Windows).
///
/// Uses `-quality` (speed/quality tradeoff) and target bitrate.
fn build_amf_args(args: &mut Vec<String>, codec: &VideoCodec, quality: &QualityPreset) {
    let (bitrate, amf_q) = match quality {
        QualityPreset::High => ("15M", "quality"),
        QualityPreset::Medium => ("8M", "balanced"),
        QualityPreset::Low => ("4M", "speed"),
    };
    args.push("-b:v".to_string());
    args.push(bitrate.to_string());
    args.push("-quality".to_string());
    args.push(amf_q.to_string());
    args.push("-pix_fmt".to_string());
    args.push(codec.ffmpeg_pix_fmt().to_string());
}

/// Intel QuickSync: h264_qsv / hevc_qsv / av1_qsv / vp9_qsv (Windows & Linux).
///
/// Uses `-global_quality` (similar to CRF, 1–51) and a preset.
fn build_qsv_args(args: &mut Vec<String>, codec: &VideoCodec, quality: &QualityPreset) {
    let global_q = match quality {
        QualityPreset::High => "18",
        QualityPreset::Medium => "23",
        QualityPreset::Low => "28",
    };
    let preset = match quality {
        QualityPreset::High => "medium",
        QualityPreset::Medium => "fast",
        QualityPreset::Low => "veryfast",
    };

    args.push("-global_quality".to_string());
    args.push(global_q.to_string());
    args.push("-preset".to_string());
    args.push(preset.to_string());
    args.push("-pix_fmt".to_string());
    args.push(codec.ffmpeg_pix_fmt().to_string());
}

/// VAAPI: h264_vaapi / hevc_vaapi / av1_vaapi / vp9_vaapi (Linux).
///
/// Uses `-qp` (quantization parameter, like CRF) and target bitrate.
fn build_vaapi_args(args: &mut Vec<String>, codec: &VideoCodec, quality: &QualityPreset) {
    let qp = match quality {
        QualityPreset::High => "18",
        QualityPreset::Medium => "23",
        QualityPreset::Low => "28",
    };
    args.push("-qp".to_string());
    args.push(qp.to_string());
    args.push("-pix_fmt".to_string());
    args.push(codec.ffmpeg_pix_fmt().to_string());
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
