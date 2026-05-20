use eframe::egui;
mod frame_encoder;
mod preview_target;

use frame_encoder::FrameEncoder;
use preview_target::PreviewTarget;
use std::collections::HashMap;
use std::sync::Arc;

/// GPU→CPU 读回缓冲区。
struct StagingBuffer {
    buffer: wgpu::Buffer,
    padded_bytes_per_row: u32,
    unpadded_bytes_per_row: u32,
    width: u32,
    height: u32,
}

/// 三重缓冲环，实现 GPU 渲染与 CPU 读回的流水线并行。
///
/// - 3 个 staging buffer 轮转使用
/// - submit 后立即 map_async，不等待
/// - try_read 非阻塞检查最早提交的 buffer 是否就绪
/// - 当 3 个都在飞行中时，必须 wait_read 释放一个
struct StagingRing {
    slots: [StagingSlot; 3],
    next_write: usize,
    next_read: usize,
    inflight: usize,
    device: wgpu::Device,
}

struct StagingSlot {
    buffer: Option<StagingBuffer>,
    /// map_async 完成通知
    rx: Option<std::sync::mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
}

impl StagingRing {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let make_slot = || StagingSlot {
            buffer: Some(Self::create_staging_buffer(device, width, height)),
            rx: None,
        };
        Self {
            slots: [make_slot(), make_slot(), make_slot()],
            next_write: 0,
            next_read: 0,
            inflight: 0,
            device: device.clone(),
        }
    }

    fn create_staging_buffer(device: &wgpu::Device, width: u32, height: u32) -> StagingBuffer {
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let buffer_size = (padded_bytes_per_row * height) as u64;

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging_ring"),
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

    fn ensure_size(&mut self, width: u32, height: u32) {
        let mut changed = false;
        for slot in &mut self.slots {
            if let Some(ref buf) = slot.buffer {
                if buf.width != width || buf.height != height {
                    // 注意：如果 buffer 正被 mapped，此处 drop 会触发 validation error。
                    // 但 ensure_size 仅在渲染前调用，此时所有 buffer 应处于 unmapped 状态。
                    slot.buffer = Some(Self::create_staging_buffer(&self.device, width, height));
                    slot.rx = None;
                    changed = true;
                }
            }
        }
        if changed {
            self.next_write = 0;
            self.next_read = 0;
            self.inflight = 0;
        }
    }

    fn can_write(&self) -> bool {
        self.inflight < 3
    }

    fn has_pending(&self) -> bool {
        self.inflight > 0
    }

    /// 获取下一个可写入的 slot 索引。调用者需先确保 can_write()。
    fn acquire_write_slot(&mut self) -> usize {
        debug_assert!(self.can_write());
        let idx = self.next_write;
        self.next_write = (self.next_write + 1) % 3;
        idx
    }

    /// 获取当前写入槽的 buffer 引用（用于 copy_texture_to_buffer）。
    fn write_slot_buffer(&self, slot_idx: usize) -> &StagingBuffer {
        self.slots[slot_idx]
            .buffer
            .as_ref()
            .expect("slot has buffer")
    }

    /// submit 之后调用：启动异步映射。
    fn map_after_submit(&mut self, slot_idx: usize) {
        let slot = &mut self.slots[slot_idx];
        let buf = slot.buffer.as_ref().expect("slot has buffer");
        let slice = buf.buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        slot.rx = Some(rx);
        self.inflight += 1;
    }

    /// 非阻塞尝试读取最早提交的缓冲区。
    ///
    /// 内部调用 device.poll() 以触发 map_async 回调。
    fn try_read(&mut self) -> Option<Vec<u8>> {
        if self.inflight == 0 {
            return None;
        }
        // Poll 触发 wgpu 的 map_async 回调
        let _ = self.device.poll(wgpu::PollType::Poll);
        let slot = &self.slots[self.next_read];
        if let Some(ref rx) = slot.rx {
            if rx.try_recv().is_ok() {
                return Some(self.finish_read());
            }
        }
        None
    }

    /// 阻塞等待最早提交的缓冲区就绪。
    #[allow(dead_code)]
    fn wait_read(&mut self) -> Vec<u8> {
        if self.inflight == 0 {
            return Vec::new();
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            {
                let slot = &self.slots[self.next_read];
                if let Some(ref rx) = slot.rx {
                    if rx.try_recv().is_ok() {
                        return self.finish_read();
                    }
                }
            }
            if std::time::Instant::now() >= deadline {
                return Vec::new();
            }
            let _ = self.device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
        }
    }

    /// 完成读取：复制数据、unmap、推进 read 指针。
    fn finish_read(&mut self) -> Vec<u8> {
        let slot = &mut self.slots[self.next_read];
        slot.rx = None;
        let buf = slot.buffer.as_ref().expect("slot has buffer");

        let data = buf.buffer.slice(..).get_mapped_range();
        let total_unpadded = (buf.unpadded_bytes_per_row * buf.height) as usize;
        let mut result = Vec::with_capacity(total_unpadded);

        // 快速路径：无行尾 padding 时直接整块复制
        if buf.padded_bytes_per_row == buf.unpadded_bytes_per_row {
            result.extend_from_slice(&data[..total_unpadded]);
        } else {
            // 慢速路径：逐行跳过 padding
            for row in 0..buf.height {
                let start = (row * buf.padded_bytes_per_row) as usize;
                let end = start + buf.unpadded_bytes_per_row as usize;
                result.extend_from_slice(&data[start..end]);
            }
        }
        drop(data);
        buf.buffer.unmap();

        self.next_read = (self.next_read + 1) % 3;
        self.inflight -= 1;
        result
    }
}

pub struct RenderContext {
    wgpu_state: Arc<eframe::egui_wgpu::RenderState>,
    preview: PreviewTarget,
    frame_encoder: FrameEncoder,
    staging_ring: StagingRing,
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
            staging_ring: StagingRing::new(&device, width, height),
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
        self.staging_ring.ensure_size(width, height);
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

    pub fn get_or_create_renderer(
        &mut self,
        clip_id: usize,
        midi_idx: usize,
        midi: &dyn nezha_renderer::NoteSource,
        _width: u32,
        _equal_key_width: bool,
    ) -> &mut nezha_renderer::Renderer {
        // Ensure seek index is built (shared across clips using the same MIDI)
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

    /// 提供对指定 clip 的 waterfall renderer 和当前 encoder 的可变引用。
    ///
    /// 用于将 waterfall 渲染接入 [`nezha_compositor::Compositor`] 的统一接口。
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

    // ========================================================================
    // Triple-buffered staging ring API（用于导出流水线）
    /// 三重缓冲是否还有空闲槽位可供渲染。
    pub fn staging_can_write(&self) -> bool {
        self.staging_ring.can_write()
    }

    /// 是否有已提交但未读回的帧。
    pub fn staging_has_pending(&self) -> bool {
        self.staging_ring.has_pending()
    }

    /// 将当前预览画面拷贝到 staging ring 的下一个空闲槽。
    ///
    /// 拷贝命令追加到当前 frame encoder，不单独提交。
    /// 调用者需先确保 `staging_can_write()` 返回 true。
    pub fn copy_frame_to_staging_ring(&mut self, _width: u32, _height: u32) -> usize {
        let slot_idx = self.staging_ring.acquire_write_slot();
        let staging = self.staging_ring.write_slot_buffer(slot_idx);
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

        slot_idx
    }

    /// 提交当前 frame encoder 并启动异步 GPU 读回。
    ///
    /// 传入 copy_frame_to_staging_ring 返回的 slot_idx。
    /// 提交后该 slot 进入"飞行中"状态，可通过 try_read_staging / wait_read_staging 获取数据。
    pub fn submit_and_map_staging(&mut self, slot_idx: usize) {
        self.frame_encoder.finish(&self.wgpu_state.queue);
        self.staging_ring.map_after_submit(slot_idx);
    }

    /// 非阻塞尝试读取最早提交的 staging buffer。
    ///
    /// 若数据就绪则返回 BGRA 像素数据，否则返回 None。
    pub fn try_read_staging(&mut self) -> Option<Vec<u8>> {
        self.staging_ring.try_read()
    }

    /// 阻塞等待最早提交的 staging buffer 就绪并读回。
    ///
    /// 若 GPU 超时（5s）则返回空 Vec。
    #[allow(dead_code)]
    pub fn wait_read_staging(&mut self) -> Vec<u8> {
        self.staging_ring.wait_read()
    }
}
