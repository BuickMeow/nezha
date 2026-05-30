use std::path::Path;
use std::process::Command;

use image::GenericImageView;

use crate::MediaError;
use crate::ffmpeg_util::ffmpeg_path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaType {
    Video,
    Audio,
    Image,
}

#[derive(Clone, Debug)]
pub struct MediaInfo {
    pub media_type: MediaType,
    pub path: String,
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub has_audio: bool,
}

pub fn probe_media(path: &str) -> Result<MediaInfo, MediaError> {
    let p = Path::new(path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if is_image_extension(&ext) {
        return probe_image(path);
    }

    probe_av(path, &ext)
}

fn is_image_extension(ext: &str) -> bool {
    matches!(
        ext,
        "png" | "jpg" | "jpeg" | "bmp" | "webp" | "tiff" | "tif" | "gif"
    )
}

fn is_audio_extension(ext: &str) -> bool {
    matches!(
        ext,
        "mp3" | "wav" | "flac" | "ogg" | "aac" | "m4a" | "wma" | "opus"
    )
}

fn probe_image(path: &str) -> Result<MediaInfo, MediaError> {
    let img = image::open(path)?;
    let (w, h) = img.dimensions();
    Ok(MediaInfo {
        media_type: MediaType::Image,
        path: path.to_string(),
        duration_secs: 5.0,
        width: w,
        height: h,
        fps: 0.0,
        has_audio: false,
    })
}

fn probe_av(path: &str, ext: &str) -> Result<MediaInfo, MediaError> {
    let ffmpeg = ffmpeg_path()?;

    let output = Command::new(&ffmpeg)
        .args(["-i", path])
        .output()
        .map_err(|_| MediaError::FfmpegNotFound)?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    let duration = parse_duration(&stderr);
    let (width, height, fps, has_video) = parse_video_stream(&stderr);
    let has_audio = stderr.contains("Audio:");

    let media_type = if has_video {
        MediaType::Video
    } else if has_audio || is_audio_extension(ext) {
        MediaType::Audio
    } else {
        return Err(MediaError::UnsupportedType);
    };

    Ok(MediaInfo {
        media_type,
        path: path.to_string(),
        duration_secs: duration,
        width,
        height,
        fps,
        has_audio,
    })
}

fn parse_duration(stderr: &str) -> f64 {
    for line in stderr.lines() {
        let trimmed = line.trim();
        if let Some(idx) = trimmed.find("Duration:") {
            let rest = &trimmed[idx + 9..].trim();
            if let Some(comma) = rest.find(',') {
                let time_str = rest[..comma].trim();
                return parse_hhmmss(time_str);
            }
        }
    }
    0.0
}

fn parse_hhmmss(s: &str) -> f64 {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return 0.0;
    }
    let hours = parts[0].parse::<f64>().unwrap_or(0.0);
    let minutes = parts[1].parse::<f64>().unwrap_or(0.0);
    let seconds = parts[2].parse::<f64>().unwrap_or(0.0);
    hours * 3600.0 + minutes * 60.0 + seconds
}

fn parse_video_stream(stderr: &str) -> (u32, u32, f64, bool) {
    for line in stderr.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("Video:") {
            continue;
        }

        let width;
        let height;
        if let Some((w, h)) = parse_resolution(trimmed) {
            width = w;
            height = h;
        } else {
            continue;
        }

        let fps = parse_fps_from_stream(trimmed);
        return (width, height, fps, true);
    }
    (0, 0, 0.0, false)
}

fn parse_resolution(line: &str) -> Option<(u32, u32)> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < len && bytes[i] == b'x' {
                i += 1;
                let w_start = start;
                let w_end = i - 1;
                let h_start = i;
                while i < len && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let h_end = i;
                if w_end > w_start && h_end > h_start {
                    let w = std::str::from_utf8(&bytes[w_start..w_end])
                        .ok()
                        .and_then(|s| s.parse::<u32>().ok());
                    let h = std::str::from_utf8(&bytes[h_start..h_end])
                        .ok()
                        .and_then(|s| s.parse::<u32>().ok());
                    if let (Some(w), Some(h)) = (w, h) {
                        if w > 0 && h > 0 {
                            return Some((w, h));
                        }
                    }
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

fn parse_fps_from_stream(line: &str) -> f64 {
    let parts: Vec<&str> = line.split_whitespace().collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "fps," || *part == "fps" {
            if i > 0 {
                if let Ok(fps) = parts[i - 1].parse::<f64>() {
                    if fps > 0.0 {
                        return fps;
                    }
                }
            }
        }
    }
    0.0
}
