use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Container {
    Mp4,
    Mov,
    Mkv,
    Avi,
}

impl Container {
    pub fn extension(&self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Mov => "mov",
            Container::Mkv => "mkv",
            Container::Avi => "avi",
        }
    }

    pub fn ffmpeg_muxer(&self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Mov => "mov",
            Container::Mkv => "matroska",
            Container::Avi => "avi",
        }
    }
}

impl std::str::FromStr for Container {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "MP4" => Ok(Container::Mp4),
            "MOV" => Ok(Container::Mov),
            "MKV" => Ok(Container::Mkv),
            "AVI" => Ok(Container::Avi),
            _ => Err(format!("unknown container: {}", s)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    H265,
    ProRes,
    Vp9,
    Av1,
}

impl VideoCodec {
    pub fn ffmpeg_encoder(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "libx264",
            VideoCodec::H265 => "libx265",
            VideoCodec::ProRes => "prores_ks",
            VideoCodec::Vp9 => "libvpx-vp9",
            VideoCodec::Av1 => "libsvtav1",
        }
    }

    pub fn ffmpeg_pix_fmt(&self) -> &'static str {
        match self {
            VideoCodec::ProRes => "yuv422p",
            _ => "yuv420p",
        }
    }
}

impl std::str::FromStr for VideoCodec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "H.264" => Ok(VideoCodec::H264),
            "H.265 / HEVC" => Ok(VideoCodec::H265),
            "ProRes" => Ok(VideoCodec::ProRes),
            "VP9" => Ok(VideoCodec::Vp9),
            "AV1" => Ok(VideoCodec::Av1),
            _ => Err(format!("unknown codec: {}", s)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QualityPreset {
    High,
    Medium,
    Low,
}

impl QualityPreset {
    pub fn crf(&self) -> &'static str {
        match self {
            QualityPreset::High => "18",
            QualityPreset::Medium => "23",
            QualityPreset::Low => "28",
        }
    }

    pub fn preset(&self) -> &'static str {
        match self {
            QualityPreset::High => "slow",
            QualityPreset::Medium => "medium",
            QualityPreset::Low => "veryfast",
        }
    }
}

impl Default for QualityPreset {
    fn default() -> Self {
        QualityPreset::Medium
    }
}

pub struct ExportConfig {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub container: Container,
    pub codec: VideoCodec,
    pub output_path: PathBuf,
    pub quality: QualityPreset,
}

impl ExportConfig {
    pub fn total_frames(&self, duration_secs: f64) -> u64 {
        (duration_secs * self.fps).ceil() as u64
    }
}
