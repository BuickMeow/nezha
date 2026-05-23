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

// ---------------------------------------------------------------------------
// Video codec
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    H265,
    ProRes,
    Vp9,
    Av1,
}

impl VideoCodec {
    /// Short codec identifier used in ffmpeg encoder names (e.g. "h264", "hevc").
    pub fn ffmpeg_codec_name(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "h264",
            VideoCodec::H265 => "hevc",
            VideoCodec::ProRes => "prores",
            VideoCodec::Vp9 => "vp9",
            VideoCodec::Av1 => "av1",
        }
    }

    /// Software-only encoder name (e.g. "libx264", "libx265").
    pub fn ffmpeg_software_encoder(&self) -> &'static str {
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

    /// Display name used in UI dropdown.
    pub fn display_name(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "H.264",
            VideoCodec::H265 => "H.265 / HEVC",
            VideoCodec::ProRes => "ProRes",
            VideoCodec::Vp9 => "VP9",
            VideoCodec::Av1 => "AV1",
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

// ---------------------------------------------------------------------------
// Encoder backend (platform-specific hardware acceleration)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncoderBackend {
    /// Pure software encoding (libx264, libx265, libvpx-vp9, libsvtav1, prores_ks).
    Software,
    /// macOS VideoToolbox (h264_videotoolbox, hevc_videotoolbox, prores_videotoolbox).
    VideoToolbox,
    /// NVIDIA NVENC (h264_nvenc, hevc_nvenc, av1_nvenc) — Windows/Linux.
    Nvenc,
    /// AMD AMF (h264_amf, hevc_amf, av1_amf) — Windows.
    Amf,
    /// Intel QuickSync (h264_qsv, hevc_qsv, av1_qsv, vp9_qsv) — Windows/Linux.
    Qsv,
    /// VAAPI (h264_vaapi, hevc_vaapi, av1_vaapi, vp9_vaapi) — Linux.
    Vaapi,
}

impl EncoderBackend {
    /// ffmpeg encoder-name suffix (e.g. "videotoolbox", "nvenc").
    pub fn ffmpeg_suffix(&self) -> Option<&'static str> {
        match self {
            EncoderBackend::Software => None,
            EncoderBackend::VideoToolbox => Some("videotoolbox"),
            EncoderBackend::Nvenc => Some("nvenc"),
            EncoderBackend::Amf => Some("amf"),
            EncoderBackend::Qsv => Some("qsv"),
            EncoderBackend::Vaapi => Some("vaapi"),
        }
    }

    pub fn is_hardware(&self) -> bool {
        !matches!(self, EncoderBackend::Software)
    }

    /// Display name shown in the UI.
    pub fn display_name(&self) -> &'static str {
        match self {
            EncoderBackend::Software => "Software (CPU)",
            EncoderBackend::VideoToolbox => "VideoToolbox (macOS)",
            EncoderBackend::Nvenc => "NVENC (NVIDIA)",
            EncoderBackend::Amf => "AMF (AMD)",
            EncoderBackend::Qsv => "QSV (Intel)",
            EncoderBackend::Vaapi => "VAAPI (Linux)",
        }
    }

    /// Return the list of backends available on the current operating system.
    pub fn available_on_current_platform() -> Vec<EncoderBackend> {
        use EncoderBackend::*;
        let mut list = vec![Software];
        // macOS
        #[cfg(target_os = "macos")]
        list.push(VideoToolbox);
        // Windows
        #[cfg(target_os = "windows")]
        {
            list.push(Nvenc);
            list.push(Amf);
            list.push(Qsv);
        }
        // Linux
        #[cfg(target_os = "linux")]
        {
            list.push(Nvenc);
            list.push(Qsv);
            list.push(Vaapi);
        }
        list
    }
}

impl std::str::FromStr for EncoderBackend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Software (CPU)" => Ok(EncoderBackend::Software),
            "VideoToolbox (macOS)" => Ok(EncoderBackend::VideoToolbox),
            "NVENC (NVIDIA)" => Ok(EncoderBackend::Nvenc),
            "AMF (AMD)" => Ok(EncoderBackend::Amf),
            "QSV (Intel)" => Ok(EncoderBackend::Qsv),
            "VAAPI (Linux)" => Ok(EncoderBackend::Vaapi),
            _ => Err(format!("unknown encoder backend: {}", s)),
        }
    }
}

// ---------------------------------------------------------------------------
// Quality preset
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum QualityPreset {
    High,
    #[default]
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

// ---------------------------------------------------------------------------
// ExportConfig
// ---------------------------------------------------------------------------

pub struct ExportConfig {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub container: Container,
    pub codec: VideoCodec,
    pub backend: EncoderBackend,
    pub output_path: PathBuf,
    pub quality: QualityPreset,
    // Optional audio data for mixing into the output
    pub audio_pcm: Option<Vec<f32>>,
    pub audio_sample_rate: u32,
    pub audio_channels: u16,
}

impl ExportConfig {
    pub fn total_frames(&self, duration_secs: f64) -> u64 {
        (duration_secs * self.fps).ceil() as u64
    }

    /// Build the full ffmpeg encoder name from codec + backend.
    ///
    /// Examples: "libx264", "h264_videotoolbox", "hevc_nvenc".
    pub fn ffmpeg_encoder_name(&self) -> String {
        match &self.backend {
            EncoderBackend::Software => self.codec.ffmpeg_software_encoder().to_string(),
            _ => {
                let codec = self.codec.ffmpeg_codec_name();
                let suffix = self
                    .backend
                    .ffmpeg_suffix()
                    .expect("hardware backend should have a suffix");
                format!("{}_{}", codec, suffix)
            }
        }
    }
}
