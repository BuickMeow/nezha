use nezha_compositor::compute_scissor_rect;
use wgpu::*;

use crate::buffer::{self, InstanceBufferSlot};
use crate::constants::MAX_INSTANCE_COUNT;
use crate::constants::MIN_INSTANCE_BUFFER_CAPACITY;
use crate::gpu_timer::GpuTimer;
use crate::instances;
use crate::keyboard;
use crate::pipeline::RenderPipelineState;
use crate::scan::NoteSeekIndex;
use crate::source::NoteSource;
use crate::state::MidiRenderState;
use crate::style::RenderStyle;
use crate::vertex::{NoteInstance, Uniforms, pack_props, pack_rgba};

#[cfg(feature = "profiling")]
macro_rules! profile_scope {
    ($name:literal) => {
        puffin::profile_scope!($name);
    };
}
#[cfg(not(feature = "profiling"))]
macro_rules! profile_scope {
    ($name:literal) => {};
}

pub struct Renderer {
    device: Device,
    queue: Queue,
    render: RenderPipelineState,
    timer: GpuTimer,
    instance_buffers: Vec<InstanceBufferSlot>,
    instance_scratch: Vec<NoteInstance>,
    cached_layouts: Vec<(f32, f32)>,
    cached_layout_width: u32,
    cached_layout_equal_key_width: bool,
    current_batch_counts: Vec<usize>,
    current_note_count: usize,
    pub state: MidiRenderState,
    pub seek_index: Option<NoteSeekIndex>,
}

impl Renderer {
    /// Create a new renderer with the given wgpu device, queue, and swap-chain format.
    pub fn new(device: Device, queue: Queue, format: TextureFormat) -> Self {
        let render_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("waterfall_shader"),
            source: ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let render = RenderPipelineState::new(&device, format, &render_shader);

        let timer = GpuTimer::new(&device, &queue);

        Self {
            device,
            queue,
            render,
            timer,
            instance_buffers: Vec::new(),
            instance_scratch: Vec::new(),
            cached_layouts: Vec::new(),
            cached_layout_width: 0,
            cached_layout_equal_key_width: false,
            current_batch_counts: Vec::new(),
            current_note_count: 0,
            state: MidiRenderState::default(),
            seek_index: None,
        }
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// Prepare rendering data (CPU computation + buffer uploads).
    ///
    /// Call this before [`Self::draw`].
    pub fn prepare(
        &mut self,
        width: u32,
        height: u32,
        time: f64,
        speed: f32,
        midi: Option<&dyn NoteSource>,
        style: &RenderStyle,
    ) {
        profile_scope!("prepare");
        let uniforms = Uniforms {
            time: time as f32,
            width: width as f32,
            height: height as f32,
            _pad: 0.0,
        };
        self.queue.write_buffer(
            &self.render.uniform_buffer,
            0,
            bytemuck::bytes_of(&uniforms),
        );

        let mut instances = std::mem::take(&mut self.instance_scratch);
        instances.clear();
        self.ensure_cached_key_layouts(width, style.equal_key_width);
        let layouts = &self.cached_layouts;

        self.current_note_count = match midi {
            Some(m) => instances::build_instances(
                &mut instances,
                layouts,
                height,
                time,
                speed,
                m,
                &mut self.state,
                self.seek_index.as_ref(),
                style,
            ),
            None => {
                instances.push(NoteInstance {
                    x: 0.0,
                    y: 0.0,
                    w: width as f32,
                    h: height as f32,
                    rgba_packed: pack_rgba(
                        style.background[0] as f32,
                        style.background[1] as f32,
                        style.background[2] as f32,
                        style.background[3] as f32,
                    ),
                    props_packed: pack_props(0.0, 0.0),
                    velocity: 0,
                    flags: 0,
                });
                0
            }
        };

        let instance_size = std::mem::size_of::<NoteInstance>() as u64;
        let batches: Vec<&[NoteInstance]> = if instances.is_empty() {
            Vec::new()
        } else {
            instances.chunks(MAX_INSTANCE_COUNT).collect()
        };

        self.current_batch_counts.clear();
        for batch in &batches {
            self.current_batch_counts.push(batch.len());
        }

        while self.instance_buffers.len() > batches.len() {
            self.instance_buffers.pop();
        }
        while self.instance_buffers.len() < batches.len() {
            self.instance_buffers
                .push(buffer::create_instance_buffer_slot(
                    &self.device,
                    instance_size,
                    MIN_INSTANCE_BUFFER_CAPACITY,
                ));
        }
        for (i, batch) in batches.iter().enumerate() {
            let required_instances = batch.len().max(1);
            if self.instance_buffers[i].capacity_instances < required_instances {
                self.instance_buffers[i] = buffer::create_instance_buffer_slot(
                    &self.device,
                    instance_size,
                    buffer::next_instance_capacity(required_instances),
                );
            }
            self.queue.write_buffer(
                &self.instance_buffers[i].buffer,
                0,
                bytemuck::cast_slice(batch),
            );
        }

        instances.clear();
        self.instance_scratch = instances;
    }

    /// Draw the prepared instances into the given target.
    ///
    /// Must be preceded by a call to [`Self::prepare`].
    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        load_op: wgpu::LoadOp<wgpu::Color>,
        rect: (f32, f32, f32, f32),
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("waterfall_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: load_op,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            multiview_mask: None,
            timestamp_writes: None,
        });

        let (sx, sy, sw, sh) = compute_scissor_rect(rect, width, height);
        pass.set_scissor_rect(sx, sy, sw, sh);

        if !self.instance_buffers.is_empty() && !self.current_batch_counts.is_empty() {
            pass.set_pipeline(&self.render.pipeline);
            pass.set_bind_group(0, &self.render.bind_group, &[]);
            for (i, &count) in self.current_batch_counts.iter().enumerate() {
                pass.set_vertex_buffer(0, self.instance_buffers[i].buffer.slice(..));
                pass.draw(0..6, 0..count as u32);
            }
        }
    }

    /// Render one frame (legacy API).
    ///
    /// Prefer using [`Self::prepare`] + [`Self::draw`] for compositor integration.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        encoder: &mut CommandEncoder,
        target: &TextureView,
        width: u32,
        height: u32,
        time: f64,
        speed: f32,
        midi: Option<&dyn NoteSource>,
        style: &RenderStyle,
        clear_background: bool,
    ) {
        profile_scope!("render");
        self.prepare(width, height, time, speed, midi, style);

        if let Some(qs) = self.timer.query_set.as_ref() {
            encoder.write_timestamp(qs, 0);
            encoder.write_timestamp(qs, 1);
        }

        let load_op = if clear_background {
            LoadOp::Clear(Color {
                r: style.background[0],
                g: style.background[1],
                b: style.background[2],
                a: style.background[3],
            })
        } else {
            LoadOp::Load
        };

        self.draw(
            encoder,
            target,
            width,
            height,
            load_op,
            (0.0, 0.0, 1.0, 1.0),
        );
        self.timer.resolve(encoder);
    }

    pub fn upload_note_data(&mut self, source: &dyn NoteSource) {
        profile_scope!("upload_note_data");
        self.seek_index = Some(NoteSeekIndex::build(source));
    }

    pub fn clear_note_data(&mut self) {
        self.seek_index = None;
        self.state.reset();
    }

    /// Whether GPU timestamp queries are supported on this device.
    pub fn gpu_timing_available(&self) -> bool {
        self.timer.supported
    }

    /// Read back GPU timestamps from the previous frame.
    /// Returns `(compute_ms, render_ms)` or `None` if unsupported or timed out.
    pub fn read_gpu_timings(&self) -> Option<(f64, f64)> {
        self.timer.read_timings(&self.device)
    }

    pub fn read_instance_overflowed(&self) -> Option<bool> {
        Some(false)
    }

    /// Total number of note instances prepared for the current frame（不含键盘琴键）。
    pub fn total_instances(&self) -> usize {
        self.current_note_count
    }

    fn ensure_cached_key_layouts(&mut self, width: u32, equal_key_width: bool) {
        if self.cached_layouts.is_empty()
            || self.cached_layout_width != width
            || self.cached_layout_equal_key_width != equal_key_width
        {
            self.cached_layouts = keyboard::compute_key_layouts(width, equal_key_width);
            self.cached_layout_width = width;
            self.cached_layout_equal_key_width = equal_key_width;
        }
    }
}

/// Adapter that wraps a [`Renderer`] as a [`LayerRenderer`] for use with the compositor.
///
/// Preparation is expected to be done externally before wrapping.
pub struct WaterfallLayer<'a> {
    pub renderer: &'a Renderer,
}

impl<'a> nezha_compositor::LayerRenderer for WaterfallLayer<'a> {
    fn prepare(&mut self, _width: u32, _height: u32, _time: f64) {}

    fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        _time: f64,
        load_op: wgpu::LoadOp<wgpu::Color>,
        _blend_mode: nezha_compositor::BlendMode,
        rect: (f32, f32, f32, f32),
    ) {
        self.renderer
            .draw(encoder, target, width, height, load_op, rect);
    }
}
