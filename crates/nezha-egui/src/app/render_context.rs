use eframe::egui;
use std::collections::HashMap;
use std::sync::Arc;

pub struct RenderContext {
    wgpu_state: Arc<eframe::egui_wgpu::RenderState>,
    renderer: nezha_renderer::Renderer,
    _preview_texture: wgpu::Texture,
    pub preview_view: wgpu::TextureView,
    pub preview_texture_id: egui::TextureId,
    /// 每个 MIDI 文件索引独立的渲染状态（用于多图层叠加）
    midi_states: HashMap<usize, nezha_renderer::MidiRenderState>,
}

impl RenderContext {
    pub fn new(cc: &eframe::CreationContext<'_>, width: u32, height: u32) -> Self {
        let wgpu_state = cc.wgpu_render_state.clone().expect("wgpu backend required");
        let device = &wgpu_state.device;
        let format = wgpu_state.target_format;
        let renderer =
            nezha_renderer::Renderer::new(device.clone(), wgpu_state.queue.clone(), format);

        let (preview_texture, preview_view, preview_texture_id) = Self::create_preview(
            device,
            &mut wgpu_state.renderer.write(),
            format,
            width,
            height,
        );

        Self {
            wgpu_state: wgpu_state.into(),
            renderer,
            _preview_texture: preview_texture,
            preview_view,
            preview_texture_id,
            midi_states: HashMap::new(),
        }
    }

    fn create_preview(
        device: &wgpu::Device,
        egui_renderer: &mut eframe::egui_wgpu::Renderer,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView, egui::TextureId) {
        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let preview_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("preview_texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let preview_view = preview_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let preview_texture_id =
            egui_renderer.register_native_texture(device, &preview_view, wgpu::FilterMode::Linear);
        (preview_texture, preview_view, preview_texture_id)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let format = self.wgpu_state.target_format;
        let device = &self.wgpu_state.device;
        let mut egui_renderer = self.wgpu_state.renderer.write();
        let (tex, view, id) =
            Self::create_preview(device, &mut egui_renderer, format, width, height);
        egui_renderer.free_texture(&self.preview_texture_id);
        self._preview_texture = tex;
        self.preview_view = view;
        self.preview_texture_id = id;
    }

    /// 渲染单个图层
    pub fn render_layer(
        &mut self,
        width: u32,
        height: u32,
        time: f64,
        speed: f32,
        midi_idx: usize,
        midi: &dyn nezha_renderer::NoteSource,
        style: &nezha_renderer::RenderStyle,
        clear_background: bool,
    ) {
        let state = self.midi_states.entry(midi_idx).or_default();
        self.renderer.render(
            &self.preview_view,
            width,
            height,
            time,
            speed,
            Some(midi),
            state,
            style,
            clear_background,
        );
    }

    /// 渲染纯背景（无音符，仅清除/填充颜色）
    pub fn render_background(
        &mut self,
        width: u32,
        height: u32,
        style: &nezha_renderer::RenderStyle,
    ) {
        // 用一个虚拟的空状态来清除背景
        let mut dummy_state = nezha_renderer::MidiRenderState::default();
        self.renderer.render(
            &self.preview_view,
            width,
            height,
            0.0,
            1.0,
            None,
            &mut dummy_state,
            style,
            true,
        );
    }

    pub fn reset_midi_state(&mut self) {
        self.midi_states.clear();
    }
}
