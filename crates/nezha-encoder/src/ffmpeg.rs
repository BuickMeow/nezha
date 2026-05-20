use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::config::{ExportConfig, VideoCodec};

#[derive(Debug)]
pub enum EncoderError {
    Io(std::io::Error),
    FfmpegFailed(Option<i32>),
    FfmpegNotFound,
}

impl std::fmt::Display for EncoderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncoderError::Io(e) => write!(f, "IO error: {}", e),
            EncoderError::FfmpegFailed(code) => {
                write!(f, "ffmpeg exited with code {:?}", code)
            }
            EncoderError::FfmpegNotFound => write!(f, "ffmpeg not found"),
        }
    }
}

impl std::error::Error for EncoderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EncoderError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for EncoderError {
    fn from(e: std::io::Error) -> Self {
        EncoderError::Io(e)
    }
}

#[derive(Debug)]
pub struct FfmpegEncoder {
    process: std::process::Child,
    sender: Option<std::sync::mpsc::SyncSender<Vec<u8>>>,
    join_handle: Option<std::thread::JoinHandle<Result<(), EncoderError>>>,
}

impl FfmpegEncoder {
    pub fn new(config: &ExportConfig) -> Result<Self, EncoderError> {
        let ffmpeg = ffmpeg_path();

        // 如果指向一个具体的 sidecar 路径且文件不存在，报错
        let is_bundled_path = ffmpeg
            .file_name()
            .map_or(false, |n| n == "ffmpeg" || n == "ffmpeg.exe")
            && ffmpeg.parent().map_or(false, |p| p != PathBuf::from(""));
        if is_bundled_path && !ffmpeg.exists() {
            return Err(EncoderError::FfmpegNotFound);
        }

        let args = build_ffmpeg_args(config);

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
        // 关闭 sender，后台线程的 rx 会结束，stdin 被关闭
        self.sender.take();

        // 等待后台线程把所有剩余数据写入 ffmpeg stdin
        if let Some(handle) = self.join_handle.take() {
            handle
                .join()
                .map_err(|_| EncoderError::FfmpegFailed(None))??;
        }

        // 等待 ffmpeg 子进程结束
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
        Ok(())
    }
}

impl Drop for FfmpegEncoder {
    fn drop(&mut self) {
        // 如果用户取消导出，强制终止 ffmpeg 子进程
        let _ = self.process.kill();
        let _ = self.process.wait();
        // 等待后台线程退出（pipe broken 后自然结束）
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

/// 返回 ffmpeg 可执行文件的绝对路径。
///
/// 查找顺序：
/// 1. 当前可执行文件所在目录的 sidecar（ffmpeg / ffmpeg.exe）
/// 2. PATH 中的 ffmpeg
pub fn ffmpeg_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
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
    }

    if cfg!(target_os = "windows") {
        PathBuf::from("ffmpeg.exe")
    } else {
        PathBuf::from("ffmpeg")
    }
}

fn build_ffmpeg_args(config: &ExportConfig) -> Vec<String> {
    let mut args = Vec::new();

    // 输入：从 stdin 读取 rawvideo（BGRA，与 wgpu Bgra8UnormSrgb 一致）
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

    // 编码器
    args.push("-c:v".to_string());
    args.push(config.codec.ffmpeg_encoder().to_string());

    // 质量与像素格式设置
    match &config.codec {
        VideoCodec::H264 | VideoCodec::H265 => {
            args.push("-crf".to_string());
            args.push(config.quality.crf().to_string());
            args.push("-preset".to_string());
            args.push(config.quality.preset().to_string());
            args.push("-pix_fmt".to_string());
            args.push("yuv420p".to_string());

            if config.codec == VideoCodec::H264 {
                // 不启用 faststart（需二次处理，大文件耗时数分钟）。
                // 用户如需 web 渐进式下载，可事后运行：
                //   ffmpeg -i in.mp4 -c copy -movflags +faststart out.mp4
            }
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
            args.push("3".to_string()); // 422
            args.push("-pix_fmt".to_string());
            args.push("yuv422p".to_string());
            args.push("-qscale:v".to_string());
            args.push("9".to_string());
        }
    }

    // 容器格式
    args.push("-f".to_string());
    args.push(config.container.ffmpeg_muxer().to_string());

    // 覆盖输出文件
    args.push("-y".to_string());

    // 输出路径
    args.push(config.output_path.to_string_lossy().to_string());

    args
}
