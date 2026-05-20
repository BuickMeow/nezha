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
        clear_background: bool,
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
            clear_background,
        );
    }

    pub fn reset_midi_state(&mut self) {
        self.midi_cache.clear(&mut self.renderer);
    }

    /// 将当前预览画面的内容读回到 CPU 内存。
    ///
    /// 返回的 buffer 按行优先、每像素 4 字节 BGRA 排列，无行尾 padding。
    /// 若 GPU 映射超时则返回空 Vec。
    pub fn read_frame_bytes(&self) -> Vec<u8> {
        let device = &self.wgpu_state.device;
        let queue = &self.wgpu_state.queue;
        let width = self.preview.width();
        let height = self.preview.height();

        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;

        let buffer_size = (padded_bytes_per_row * height) as u64;

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame_readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame_copy_encoder"),
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: self.preview.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(std::iter::once(encoder.finish()));

        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut done = false;
        while std::time::Instant::now() < deadline && !done {
            let _ = device.poll(wgpu::PollType::Poll);
            done = rx.try_recv().is_ok();
            if !done {
                std::thread::yield_now();
            }
        }

        if !done {
            return Vec::new();
        }

        let data = slice.get_mapped_range();
        let mut result = Vec::with_capacity((unpadded_bytes_per_row * height) as usize);
        for row in 0..height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            result.extend_from_slice(&data[start..end]);
        }
        drop(data);
        buffer.unmap();

        result
    }
}
