use wgpu::*;

use crate::gpu_timer::GpuTimer;
use crate::keyboard;
use crate::pipeline::RenderPipelineState;
use crate::source::NoteSource;
use crate::state::MidiRenderState;
use crate::style::{RenderMode, RenderStyle};
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
    instance_buffers: Vec<Buffer>,
}

/// CPU path keeps multi-buffer batching to avoid a single hard cap per frame.
const MAX_INSTANCE_COUNT: usize = 6_000_000;

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
        }
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// Render one frame.
    pub fn render(
        &mut self,
        encoder: &mut CommandEncoder,
        target: &TextureView,
        width: u32,
        height: u32,
        time: f64,
        speed: f32,
        midi: Option<&dyn NoteSource>,
        render_state: &mut MidiRenderState,
        _note_data_id: Option<usize>,
        style: &RenderStyle,
        clear_background: bool,
    ) {
        profile_scope!("render");
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

        let instances = midi
            .map(|m| self.build_instances(width, height, time, speed, m, render_state, style))
            .unwrap_or_else(|| {
                vec![NoteInstance {
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
                }]
            });

        let instance_size = std::mem::size_of::<NoteInstance>() as u64;
        let batches: Vec<&[NoteInstance]> = if instances.is_empty() {
            Vec::new()
        } else {
            instances.chunks(MAX_INSTANCE_COUNT).collect()
        };

        while self.instance_buffers.len() > batches.len() {
            self.instance_buffers.pop();
        }
        while self.instance_buffers.len() < batches.len() {
            let buf = self.device.create_buffer(&BufferDescriptor {
                label: Some("instance_buffer"),
                size: MAX_INSTANCE_COUNT as u64 * instance_size,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_buffers.push(buf);
        }
        for (i, batch) in batches.iter().enumerate() {
            self.queue
                .write_buffer(&self.instance_buffers[i], 0, bytemuck::cast_slice(batch));
        }

        if self.timer.query_set.is_some() {
            let cpu_scope = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("cpu_build_scope"),
                timestamp_writes: self.timer.query_set.as_ref().map(|qs| {
                    ComputePassTimestampWrites {
                        query_set: qs,
                        beginning_of_pass_write_index: Some(0),
                        end_of_pass_write_index: Some(1),
                    }
                }),
            });
            drop(cpu_scope);
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

        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("render_pass"),
                timestamp_writes: self.timer.query_set.as_ref().map(|qs| RenderPassTimestampWrites {
                    query_set: qs,
                    beginning_of_pass_write_index: Some(2),
                    end_of_pass_write_index: Some(3),
                }),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: load_op,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if !batches.is_empty() {
                pass.set_pipeline(&self.render.pipeline);
                pass.set_bind_group(0, &self.render.bind_group, &[]);
                for (i, batch) in batches.iter().enumerate() {
                    pass.set_vertex_buffer(0, self.instance_buffers[i].slice(..));
                    pass.draw(0..6, 0..batch.len() as u32);
                }
            }
        }

        self.timer.resolve(encoder);
    }

    pub fn upload_note_data(
        &mut self,
        _id: usize,
        _source: &dyn NoteSource,
        _width: u32,
        _equal_key_width: bool,
    ) {
        profile_scope!("upload_note_data");
    }

    pub fn remove_note_data(&mut self, _id: usize) {}

    pub fn clear_note_data(&mut self) {}

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

    fn build_instances(
        &self,
        width: u32,
        height: u32,
        time: f64,
        speed: f32,
        midi: &dyn NoteSource,
        state: &mut MidiRenderState,
        style: &RenderStyle,
    ) -> Vec<NoteInstance> {
        let mut instances = match style.render_mode {
            RenderMode::TimeBased => {
                self.build_instances_time(width, height, time, speed, midi, state, style)
            }
            RenderMode::TickBased => {
                self.build_instances_tick(width, height, time, speed, midi, state, style)
            }
        };
        if style.keyboard_height > 0.0 {
            let mut keys = keyboard::build_keyboard_instances(
                width,
                height,
                time,
                midi,
                style.keyboard_height,
                style.equal_key_width,
                &style.palette,
                state,
            );
            instances.append(&mut keys);
        }
        instances
    }

    fn build_instances_time(
        &self,
        width: u32,
        height: u32,
        time: f64,
        speed: f32,
        midi: &dyn NoteSource,
        state: &mut MidiRenderState,
        style: &RenderStyle,
    ) -> Vec<NoteInstance> {
        let kh = (style.keyboard_height as f64).max(0.0);
        let effective_h = (height as f64 - kh).max(1.0);
        let pps = 200.0f64 * speed.max(0.01) as f64;
        let screen_top = effective_h + time * pps;
        let time_top = time + effective_h / pps + 1.0;
        let time_bottom = time;

        Self::advance_scan_indices(midi, state, time, 0.0, RenderMode::TimeBased);

        let layouts = keyboard::compute_key_layouts(width, style.equal_key_width);
        let mut instances = Vec::new();

        for key in Self::iter_render_keys(style.equal_key_width) {
            let notes = midi.key_notes(key);
            if notes.is_empty() {
                continue;
            }
            let scan = state.scan_indices[key as usize];
            let (x, w) = layouts[key as usize];

            for note in &notes[scan..] {
                if note.start > time_top {
                    break;
                }
                if note.end <= time_bottom {
                    continue;
                }

                let note_bottom = (screen_top - note.start * pps) as f32;
                let note_top = (screen_top - note.end * pps) as f32;
                let h = (note_bottom - note_top).max(1.0);

                let trk = note.track as usize % 128;
                let [r, g, b] = style.palette[trk];
                instances.push(NoteInstance {
                    x,
                    y: note_top,
                    w,
                    h,
                    rgba_packed: pack_rgba(r, g, b, 1.0),
                    props_packed: pack_props(style.rounding * f32::min(w, h), style.border_width * w / 2.0),
                    velocity: note.velocity as u32,
                    flags: 0,
                });
            }
        }

        instances
    }

    fn build_instances_tick(
        &self,
        width: u32,
        height: u32,
        time: f64,
        speed: f32,
        midi: &dyn NoteSource,
        state: &mut MidiRenderState,
        style: &RenderStyle,
    ) -> Vec<NoteInstance> {
        let kh = (style.keyboard_height as f64).max(0.0);
        let effective_h = (height as f64 - kh).max(1.0);
        let ticks_per_beat = midi.ticks_per_beat().unwrap_or(480) as f64;
        let ppt = 100.0 / ticks_per_beat * speed.max(0.01) as f64;
        let scroll_tick = midi
            .tick_at_time(time)
            .unwrap_or(time * ticks_per_beat * 2.0);
        let visible_ticks = effective_h / ppt;
        let tick_at_top = scroll_tick + visible_ticks;
        let screen_bottom = effective_h + scroll_tick * ppt;

        Self::advance_scan_indices(midi, state, time, scroll_tick, RenderMode::TickBased);

        let layouts = keyboard::compute_key_layouts(width, style.equal_key_width);
        let mut instances = Vec::new();

        for key in Self::iter_render_keys(style.equal_key_width) {
            let notes = midi.key_notes(key);
            if notes.is_empty() {
                continue;
            }
            let scan = state.scan_indices[key as usize];
            let (x, w) = layouts[key as usize];

            for note in &notes[scan..] {
                if (note.start_tick as f64) > tick_at_top + 1.0 {
                    break;
                }
                if (note.end_tick as f64) <= scroll_tick {
                    continue;
                }

                let note_top = (screen_bottom - note.end_tick as f64 * ppt) as f32;
                let note_bottom = (screen_bottom - note.start_tick as f64 * ppt) as f32;
                let h = (note_bottom - note_top).max(1.0);

                let trk = note.track as usize % 128;
                let [r, g, b] = style.palette[trk];
                instances.push(NoteInstance {
                    x,
                    y: note_top,
                    w,
                    h,
                    rgba_packed: pack_rgba(r, g, b, 1.0),
                    props_packed: pack_props(style.rounding * f32::min(w, h), style.border_width * w / 2.0),
                    velocity: note.velocity as u32,
                    flags: 0,
                });
            }
        }

        instances
    }

    fn advance_scan_indices(
        midi: &dyn NoteSource,
        state: &mut MidiRenderState,
        time: f64,
        scroll_tick: f64,
        mode: RenderMode,
    ) {
        match mode {
            RenderMode::TimeBased => {
                if time < state.last_time {
                    state.scan_indices = [0; 128];
                }
                state.last_time = time;
                for key in 0..128u8 {
                    let notes = midi.key_notes(key);
                    if notes.is_empty() {
                        continue;
                    }
                    let mut scan = state.scan_indices[key as usize];
                    while scan < notes.len() && notes[scan].end <= time {
                        scan += 1;
                    }
                    state.scan_indices[key as usize] = scan;
                }
            }
            RenderMode::TickBased => {
                if scroll_tick < state.last_scroll_tick {
                    state.scan_indices = [0; 128];
                }
                state.last_scroll_tick = scroll_tick;
                for key in 0..128u8 {
                    let notes = midi.key_notes(key);
                    if notes.is_empty() {
                        continue;
                    }
                    let mut scan = state.scan_indices[key as usize];
                    while scan < notes.len() && (notes[scan].end_tick as f64) <= scroll_tick {
                        scan += 1;
                    }
                    state.scan_indices[key as usize] = scan;
                }
            }
        }
    }

    fn iter_render_keys(equal_key_width: bool) -> Vec<u8> {
        if equal_key_width {
            (0..128u8).collect()
        } else {
            (0..128u8)
                .filter(|k| !keyboard::is_black_key(*k))
                .chain((0..128u8).filter(|k| keyboard::is_black_key(*k)))
                .collect()
        }
    }
}
