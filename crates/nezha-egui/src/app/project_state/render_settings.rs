use nezha_xsynth::ChannelCount;

use crate::app::constants;

pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub audio_sample_rate: u32,
    pub audio_channels: ChannelCount,
    pub audio_use_limiter: bool,
    pub audio_layers: u32,
    pub audio_min_velocity: u8,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: constants::DEFAULT_PREVIEW_WIDTH,
            height: constants::DEFAULT_PREVIEW_HEIGHT,
            fps: constants::DEFAULT_FPS,
            audio_sample_rate: constants::DEFAULT_AUDIO_SAMPLE_RATE,
            audio_channels: ChannelCount::Stereo,
            audio_use_limiter: true,
            audio_layers: 32,
            audio_min_velocity: 1,
        }
    }
}
