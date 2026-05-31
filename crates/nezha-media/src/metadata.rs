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
    // Find pattern like "1920x1080" by searching for 'x' surrounded by digits
    let mut search_start = 0;
    while let Some(x_pos) = line[search_start..].find('x') {
        let x_pos = search_start + x_pos;
        // Walk left to find start of width number
        let w_end = x_pos;
        let w_start = line[..w_end].rfind(|c: char| !c.is_ascii_digit()).map_or(0, |i| i + 1);
        // Walk right to find end of height number
        let h_start = x_pos + 1;
        let h_end = line[h_start..].find(|c: char| !c.is_ascii_digit()).map_or(line.len(), |i| h_start + i);
        if let (Ok(w), Ok(h)) = (line[w_start..w_end].parse::<u32>(), line[h_start..h_end].parse::<u32>()) {
            if w > 0 && h > 0 {
                return Some((w, h));
            }
        }
        search_start = x_pos + 1;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hhmmss_basic() {
        assert!((parse_hhmmss("00:03:45.67") - 225.67).abs() < 0.01);
    }

    #[test]
    fn test_parse_hhmmss_zero() {
        assert!((parse_hhmmss("00:00:00.00") - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_hhmmss_one_hour() {
        assert!((parse_hhmmss("01:00:00.00") - 3600.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_hhmmss_invalid() {
        assert_eq!(parse_hhmmss("invalid"), 0.0);
    }

    #[test]
    fn test_parse_duration_from_ffmpeg_output() {
        let stderr = "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'test.mp4':
  Duration: 00:02:30.50, start: 0.000000, bitrate: 1234 kb/s
    Stream #0:0: Video: h264, yuv420p, 1920x1080, 30 fps, 30 tbr, 15360 tbn
    Stream #0:1: Audio: aac, 44100 Hz, stereo";
        assert!((parse_duration(stderr) - 150.50).abs() < 0.01);
    }

    #[test]
    fn test_parse_duration_missing() {
        assert_eq!(parse_duration("no duration here"), 0.0);
    }

    #[test]
    fn test_parse_resolution_1080p() {
        let line = "Stream #0:0: Video: h264, yuv420p, 1920x1080 [SAR 1:1 DAR 16:9]";
        assert_eq!(parse_resolution(line), Some((1920, 1080)));
    }

    #[test]
    fn test_parse_resolution_720p() {
        let line = "Video: h264, yuv420p, 1280x720";
        assert_eq!(parse_resolution(line), Some((1280, 720)));
    }

    #[test]
    fn test_parse_resolution_4k() {
        let line = "Video: hevc, yuv420p10le, 3840x2160";
        assert_eq!(parse_resolution(line), Some((3840, 2160)));
    }

    #[test]
    fn test_parse_resolution_none() {
        let line = "Stream #0:1: Audio: aac, 44100 Hz, stereo";
        assert_eq!(parse_resolution(line), None);
    }

    #[test]
    fn test_parse_resolution_no_match() {
        let line = "some random text without resolution";
        assert_eq!(parse_resolution(line), None);
    }

    #[test]
    fn test_parse_fps_30() {
        let line = "Stream #0:0: Video: h264, yuv420p, 1920x1080, 30 fps, 30 tbr";
        assert!((parse_fps_from_stream(line) - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_fps_29_97() {
        let line = "Stream #0:0: Video: h264, yuv420p, 1920x1080, 29.97 fps, 29.97 tbr";
        assert!((parse_fps_from_stream(line) - 29.97).abs() < 0.01);
    }

    #[test]
    fn test_parse_fps_60() {
        let line = "Video: h264, yuv420p, 3840x2160, 60 fps, 60 tbr, 15360 tbn";
        assert!((parse_fps_from_stream(line) - 60.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_fps_no_fps() {
        let line = "Stream #0:1: Audio: aac, 44100 Hz, stereo";
        assert_eq!(parse_fps_from_stream(line), 0.0);
    }

    #[test]
    fn test_parse_video_stream_full() {
        let stderr = "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'test.mp4':
  Duration: 00:01:00.00, start: 0.000000, bitrate: 5000 kb/s
    Stream #0:0(und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 1920x1080 [SAR 1:1 DAR 16:9], 4500 kb/s, 29.97 fps, 29.97 tbr, 16k tbn, 59.94 tbc (default)
    Stream #0:1(und): Audio: aac (LC) (mp4a / 0x6134706D), 44100 Hz, stereo, fltp, 128 kb/s (default)";
        let (w, h, fps, has_video) = parse_video_stream(stderr);
        assert_eq!(w, 1920);
        assert_eq!(h, 1080);
        assert!((fps - 29.97).abs() < 0.01);
        assert!(has_video);
    }

    #[test]
    fn test_parse_video_stream_no_video() {
        let stderr = "Input #0, mp3, from 'test.mp3':
  Duration: 00:03:00.00, start: 0.000000, bitrate: 320 kb/s
    Stream #0: Audio: mp3, 44100 Hz, stereo, 320 kb/s";
        let (w, h, fps, has_video) = parse_video_stream(stderr);
        assert_eq!(w, 0);
        assert_eq!(h, 0);
        assert_eq!(fps, 0.0);
        assert!(!has_video);
    }

    #[test]
    fn test_is_image_extension() {
        assert!(is_image_extension("png"));
        assert!(is_image_extension("jpg"));
        assert!(is_image_extension("jpeg"));
        assert!(is_image_extension("bmp"));
        assert!(is_image_extension("webp"));
        assert!(!is_image_extension("mp4"));
        assert!(!is_image_extension("mp3"));
    }

    #[test]
    fn test_is_audio_extension() {
        assert!(is_audio_extension("mp3"));
        assert!(is_audio_extension("wav"));
        assert!(is_audio_extension("flac"));
        assert!(is_audio_extension("ogg"));
        assert!(is_audio_extension("aac"));
        assert!(!is_audio_extension("mp4"));
        assert!(!is_audio_extension("png"));
    }
}
