use eframe::egui;
use std::sync::Arc;

pub struct RenderContext {
    wgpu_state: Arc<eframe::egui_wgpu::RenderState>,
    renderer: nezha_renderer::Renderer,
    _preview_texture: wgpu::Texture,
    pub preview_view: wgpu::TextureView,
    pub preview_texture_id: egui::TextureId,
    midi_render_state: nezha_renderer::MidiRenderState,
}

impl RenderContext {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        width: u32,
        height: u32,
    ) -> Self {
        let wgpu_state = cc
            .wgpu_render_state
            .clone()
            .expect("wgpu backend required");
        let device = &wgpu_state.device;
        let format = wgpu_state.target_format;
        let renderer = nezha_renderer::Renderer::new(device.clone(), wgpu_state.queue.clone(), format);

        let (preview_texture, preview_view, preview_texture_id) =
            Self::create_preview(device, &mut wgpu_state.renderer.write(), format, width, height);

        Self {
            wgpu_state: wgpu_state.into(),
            renderer,
            _preview_texture: preview_texture,
            preview_view,
            preview_texture_id,
            midi_render_state: nezha_renderer::MidiRenderState::default(),
        }
    }

    fn create_preview(
        device: &wgpu::Device,
        egui_renderer: &mut eframe::egui_wgpu::Renderer,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView, egui::TextureId) {
        let texture_size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
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
        let preview_texture_id = egui_renderer.register_native_texture(
            device, &preview_view, wgpu::FilterMode::Linear,
        );
        (preview_texture, preview_view, preview_texture_id)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let format = self.wgpu_state.target_format;
        let device = &self.wgpu_state.device;
        let mut egui_renderer = self.wgpu_state.renderer.write();
        egui_renderer.free_texture(&self.preview_texture_id);
        let (tex, view, id) = Self::create_preview(device, &mut egui_renderer, format, width, height);
        self._preview_texture = tex;
        self.preview_view = view;
        self.preview_texture_id = id;
    }

    pub fn render(
        &mut self,
        width: u32,
        height: u32,
        time: f64,
        speed: f32,
        midi: Option<&dyn nezha_renderer::NoteSource>,
    ) {
        self.renderer.render(
            &self.preview_view, width, height, time, speed, midi,
            &mut self.midi_render_state,
        );
    }

    pub fn reset_midi_state(&mut self) {
        self.midi_render_state.reset();
    }
}
