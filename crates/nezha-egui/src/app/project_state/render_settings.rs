/// 预览与导出共享的渲染参数。
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 60,
        }
    }
}
