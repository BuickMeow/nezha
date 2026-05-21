pub mod export;
mod frame_encoder;
mod preview_target;

use frame_encoder::FrameEncoder;
use preview_target::PreviewTarget;
use std::collections::HashMap;
use std::sync::Arc;

pub struct RenderContext {
    wgpu_state: Arc<eframe::egui_wgpu::RenderState>,
    preview: PreviewTarget,
    frame_encoder: FrameEncoder,
    waterfall_renderers: HashMap<usize, nezha_renderer::Renderer>,
    seek_indices: HashMap<usize, nezha_renderer::NoteSeekIndex>,
}

impl RenderContext {
    pub fn new(cc: &eframe::CreationContext<'_>, width: u32, height: u32) -> Self {
        let wgpu_state = cc.wgpu_render_state.clone().expect("wgpu backend required");
        let device = wgpu_state.device.clone();
        let format = wgpu_state.target_format;

        let preview = PreviewTarget::new(
            &device,
            &mut wgpu_state.renderer.write(),
            format,
            width,
            height,
        );

        Self {
            wgpu_state: wgpu_state.into(),
            preview,
            frame_encoder: FrameEncoder::default(),
            waterfall_renderers: HashMap::new(),
            seek_indices: HashMap::new(),
        }
    }

    pub fn ensure_preview_size(&mut self, width: u32, height: u32) {
        let format = self.wgpu_state.target_format;
        let device = &self.wgpu_state.device;
        let mut egui_renderer = self.wgpu_state.renderer.write();
        self.preview
            .ensure_size(device, &mut egui_renderer, format, width, height);
    }

    pub fn preview_texture_id(&self) -> egui::TextureId {
        self.preview.texture_id()
    }

    pub fn preview_texture(&self) -> &wgpu::Texture {
        self.preview.texture()
    }

    pub fn begin_pass(&mut self) {
        self.frame_encoder.begin(&self.wgpu_state.device);
    }

    /// 完成当前帧并提交到 GPU。
    ///
    /// 在不涉及 staging readback 的路径（如实时预览）中使用。
    pub fn end_pass(&mut self) {
        self.frame_encoder.finish(&self.wgpu_state.queue);
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.wgpu_state.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.wgpu_state.queue
    }

    pub fn target_format(&self) -> wgpu::TextureFormat {
        self.wgpu_state.target_format
    }

    pub fn preview_view(&self) -> &wgpu::TextureView {
        self.preview.view()
    }

    pub fn encoder_mut(&mut self) -> &mut wgpu::CommandEncoder {
        self.frame_encoder.encoder_mut()
    }

    /// 取出当前 CommandEncoder 的所有权（用于导出管线）。
    ///
    /// 调用后需通过 [`Self::begin_pass`] 重建 encoder。
    pub fn take_encoder(&mut self) -> wgpu::CommandEncoder {
        self.frame_encoder.take_encoder()
    }

    pub fn get_or_create_renderer(
        &mut self,
        clip_id: usize,
        midi_idx: usize,
        midi: &dyn nezha_renderer::NoteSource,
        _width: u32,
        _equal_key_width: bool,
    ) -> &mut nezha_renderer::Renderer {
        if !self.seek_indices.contains_key(&midi_idx) {
            self.seek_indices
                .insert(midi_idx, nezha_renderer::NoteSeekIndex::build(midi));
        }
        let seek_index = self.seek_indices.get(&midi_idx).cloned();

        self.waterfall_renderers.entry(clip_id).or_insert_with(|| {
            let device = self.wgpu_state.device.clone();
            let queue = self.wgpu_state.queue.clone();
            let format = self.wgpu_state.target_format;
            let mut renderer = nezha_renderer::Renderer::new(device, queue, format);
            renderer.seek_index = seek_index;
            renderer
        })
    }

    pub fn with_waterfall_renderer(
        &mut self,
        clip_id: usize,
        f: impl FnOnce(&mut nezha_renderer::Renderer, &mut wgpu::CommandEncoder),
    ) {
        let encoder = self.frame_encoder.encoder_mut();
        let renderer = self.waterfall_renderers.get_mut(&clip_id).unwrap();
        f(renderer, encoder);
    }

    pub fn reset_midi_state(&mut self) {
        for renderer in self.waterfall_renderers.values_mut() {
            renderer.clear_note_data();
        }
        self.seek_indices.clear();
    }
}
