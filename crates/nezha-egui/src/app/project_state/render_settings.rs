use nezha_xsynth::ChannelCount;

/// 预览与导出共享的渲染参数。
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    // ── xsynth 音频渲染参数 ──
    pub audio_sample_rate: u32,
    pub audio_channels: ChannelCount,
    pub audio_use_limiter: bool,
    pub audio_layers: u32,
    /// 筛除力度 ≤ 此值的音符（默认 1）。
    pub audio_min_velocity: u8,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60,
            audio_sample_rate: 48000,
            audio_channels: ChannelCount::Stereo,
            audio_use_limiter: true,
            audio_layers: 32,
            audio_min_velocity: 1,
        }
    }
}
