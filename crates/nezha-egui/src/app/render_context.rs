use eframe::egui;
mod frame_encoder;
mod midi_cache;
mod preview_target;

use frame_encoder::FrameEncoder;
use midi_cache::MidiRenderCache;
use preview_target::PreviewTarget;
use std::sync::Arc;

/// 可复用的 GPU→CPU 读回缓冲区。
struct StagingBuffer {
    buffer: wgpu::Buffer,
    padded_bytes_per_row: u32,
    unpadded_bytes_per_row: u32,
    width: u32,
    height: u32,
}

/// Staging buffer 池，避免每帧创建/销毁 GPU buffer。
struct StagingPool {
    free: Vec<StagingBuffer>,
    pending: Option<StagingBuffer>,
}

impl StagingPool {
    fn new() -> Self {
        Self {
            free: Vec::new(),
            pending: None,
        }
    }

    fn acquire(&mut self, device: &wgpu::Device, width: u32, height: u32) -> StagingBuffer {
        // 先从空闲池找尺寸匹配的
        if let Some(idx) = self
            .free
            .iter()
            .position(|b| b.width == width && b.height == height)
        {
            self.free.swap_remove(idx)
        } else {
            let bytes_per_pixel = 4u32;
            let unpadded_bytes_per_row = width * bytes_per_pixel;
            let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
            let buffer_size = (padded_bytes_per_row * height) as u64;

            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("staging_buffer"),
                size: buffer_size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            StagingBuffer {
                buffer,
                padded_bytes_per_row,
                unpadded_bytes_per_row,
                width,
                height,
            }
        }
    }

    fn release(&mut self, buf: StagingBuffer) {
        self.free.push(buf);
    }
}

pub struct RenderContext {
    wgpu_state: Arc<eframe::egui_wgpu::RenderState>,
    renderer: nezha_renderer::Renderer,
    preview: PreviewTarget,
    midi_cache: MidiRenderCache,
    frame_encoder: FrameEncoder,
    staging_pool: StagingPool,
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
            staging_pool: StagingPool::new(),
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

    /// 将当前预览画面拷贝到 staging buffer（使用当前 frame encoder，不单独提交）。
    ///
    /// 必须在 `begin_pass()` 之后、`submit_and_read_staging()` 之前调用。
    /// 拷贝命令会被追加到当前 encoder 中，与渲染 pass 合并为一次 submit。
    pub fn copy_frame_to_staging(&mut self, width: u32, height: u32) {
        let staging = self
            .staging_pool
            .acquire(&self.wgpu_state.device, width, height);
        let encoder = self.frame_encoder.encoder_mut();

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: self.preview.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(staging.padded_bytes_per_row),
                    rows_per_image: Some(staging.height),
                },
            },
            wgpu::Extent3d {
                width: staging.width,
                height: staging.height,
                depth_or_array_layers: 1,
            },
        );

        self.staging_pool.pending = Some(staging);
    }

    /// 提交当前帧编码器（包含渲染 pass + 拷贝），并读回 staging buffer 数据。
    ///
    /// 返回 BGRA 像素数据，无行尾 padding。若 GPU 映射超时则返回空 Vec。
    pub fn submit_and_read_staging(&mut self) -> Vec<u8> {
        // 1. 提交 frame encoder（包含所有 render pass + texture copy）
        self.frame_encoder.finish(&self.wgpu_state.queue);

        // 2. 取出 pending staging buffer 并读取
        let Some(staging) = self.staging_pool.pending.take() else {
            return Vec::new();
        };

        let result = self.read_staging_data(&staging);
        self.staging_pool.release(staging);
        result
    }

    /// 同步读回 staging buffer 中的像素数据。
    ///
    /// 使用 PollType::Wait 替代 busy-poll，减少 CPU 空转。
    fn read_staging_data(&self, staging: &StagingBuffer) -> Vec<u8> {
        let device = &self.wgpu_state.device;
        let slice = staging.buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut done = false;
        while std::time::Instant::now() < deadline && !done {
            // PollType::Wait 阻塞等待 GPU 完成工作，比 Poll + yield 更高效
            let _ = device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
            done = rx.try_recv().is_ok();
        }

        if !done {
            return Vec::new();
        }

        let data = slice.get_mapped_range();
        let mut result =
            Vec::with_capacity((staging.unpadded_bytes_per_row * staging.height) as usize);
        for row in 0..staging.height {
            let start = (row * staging.padded_bytes_per_row) as usize;
            let end = start + staging.unpadded_bytes_per_row as usize;
            result.extend_from_slice(&data[start..end]);
        }
        drop(data);
        staging.buffer.unmap();

        result
    }
}
