use eframe::egui;
mod frame_encoder;
mod midi_cache;
mod preview_target;

use frame_encoder::FrameEncoder;
use midi_cache::MidiRenderCache;
use preview_target::PreviewTarget;
use std::sync::Arc;

pub struct RenderContext {
    wgpu_state: Arc<eframe::egui_wgpu::RenderState>,
    renderer: nezha_renderer::Renderer,
    preview: PreviewTarget,
    midi_cache: MidiRenderCache,
    frame_encoder: FrameEncoder,
}

impl RenderContext {
    pub fn new(cc: &eframe::CreationContext<'_>, width: u32, height: u32) -> Self {
        let wgpu_state = cc.wgpu_render_state.clone().expect("wgpu backend required");
        let device = &wgpu_state.device;
        let format = wgpu_state.target_format;
        let renderer =
            nezha_renderer::Renderer::new(device.clone(), wgpu_state.queue.clone(), format);

        let preview = PreviewTarget::new(
            device,
            &mut wgpu_state.renderer.write(),
            format,
            width,
            height,
        );

        Self {
            wgpu_state: wgpu_state.into(),
            renderer,
            preview,
            midi_cache: MidiRenderCache::default(),
            frame_encoder: FrameEncoder::default(),
        }
    }

    fn ensure_preview_size(&mut self, width: u32, height: u32) {
        let format = self.wgpu_state.target_format;
        let device = &self.wgpu_state.device;
        let mut egui_renderer = self.wgpu_state.renderer.write();
        self.preview
            .ensure_size(device, &mut egui_renderer, format, width, height);
    }

    pub fn preview_texture_id(&self) -> egui::TextureId {
        self.preview.texture_id()
    }

    pub fn begin_pass(&mut self) {
        self.frame_encoder.begin(&self.wgpu_state.device);
    }

    pub fn end_pass(&mut self) {
        self.frame_encoder.finish(&self.wgpu_state.queue);
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
        self.ensure_preview_size(width, height);
        self.midi_cache.ensure_uploaded(
            &mut self.renderer,
            width,
            midi_idx,
            midi,
            style.equal_key_width,
        );
        let encoder = self.frame_encoder.encoder_mut();
        let state = self.midi_cache.state_mut(midi_idx);
        self.renderer.render(
            encoder,
            self.preview.view(),
            width,
            height,
            time,
            speed,
            Some(midi),
            state,
            Some(midi_idx),
            style,
            clear_background,
        );
    }

    pub fn render_background(
        &mut self,
        width: u32,
        height: u32,
        style: &nezha_renderer::RenderStyle,
    ) {
        self.ensure_preview_size(width, height);
        let mut dummy_state = nezha_renderer::MidiRenderState::default();
        let encoder = self.frame_encoder.encoder_mut();
        self.renderer.render(
            encoder,
            self.preview.view(),
            width,
            height,
            0.0,
            1.0,
            None,
            &mut dummy_state,
            None,
            style,
            true,
        );
    }

    pub fn reset_midi_state(&mut self) {
        self.midi_cache.clear(&mut self.renderer);
    }
}
