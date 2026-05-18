use std::collections::HashMap;
use wgpu::*;

use crate::compute::{ComputeUniforms, GpuNote, GpuNoteBundle};
use crate::gpu_timer::GpuTimer;
use crate::keyboard;
use crate::pipeline::ComputePipelineState;
use crate::pipeline::RenderPipelineState;
use crate::source::NoteSource;
use crate::state::MidiRenderState;
use crate::style::{RenderMode, RenderStyle};
use crate::vertex::NoteInstance;

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
    compute: ComputePipelineState,
    timer: GpuTimer,

    note_bundles: HashMap<usize, GpuNoteBundle>,
    current_width: u32,
    current_equal_key_width: bool,
    cached_palette: [[f32; 3]; 128],
    /// Keyboard dirty flag — skips CPU recomputation when time/style hasn't changed
    keyboard_dirty: bool,
    cached_keyboard_time: f64,
    cached_scroll_tick: f64,
    cached_keyboard_height: f32,
}

mod chunk;
mod pass;
mod scan;

impl Renderer {
    /// Create a new renderer with the given wgpu device, queue, and swap-chain format.
    pub fn new(device: Device, queue: Queue, format: TextureFormat) -> Self {
        let render_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("waterfall_shader"),
            source: ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let compute_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("compute_notes"),
            source: ShaderSource::Wgsl(include_str!("compute_notes.wgsl").into()),
        });

        let render = RenderPipelineState::new(&device, format, &render_shader);

        let instance_size = std::mem::size_of::<NoteInstance>() as u64;
        let compute = ComputePipelineState::new(
            &device,
            &queue,
            &compute_shader,
            crate::compute::MAX_INSTANCE_COUNT as u64 * instance_size,
            128 * instance_size,
        );

        let timer = GpuTimer::new(&device, &queue);

        Self {
            device,
            queue,
            render,
            compute,
            timer,
            note_bundles: HashMap::new(),
            current_width: 0,
            current_equal_key_width: false,
            cached_palette: [[0.0; 3]; 128],
            keyboard_dirty: true,
            cached_keyboard_time: f64::NEG_INFINITY,
            cached_scroll_tick: f64::NEG_INFINITY,
            cached_keyboard_height: -1.0,
        }
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// Upload note data from a [`NoteSource`] to the GPU.
    /// Notes are automatically split into chunks to stay within GPU buffer limits.
    pub fn upload_note_data(
        &mut self,
        id: usize,
        source: &dyn NoteSource,
        width: u32,
        equal_key_width: bool,
    ) {
        profile_scope!("upload_note_data");
        Self::update_shared_key_layouts(
            &self.queue,
            &self.compute.shared_key_layouts_buf,
            width,
            equal_key_width,
        );
        self.current_width = width;
        self.current_equal_key_width = equal_key_width;

        // Flatten notes per key, compute total
        let mut key_notes: [Vec<GpuNote>; 128] = std::array::from_fn(|_| Vec::new());
        for key in 0..128u8 {
            let notes = source.key_notes(key);
            for note in notes {
                key_notes[key as usize].push(GpuNote {
                    start: note.start as f32,
                    end: note.end as f32,
                    start_tick: note.start_tick,
                    end_tick: note.end_tick,
                    track: note.track as u32,
                    velocity: note.velocity as u32,
                });
            }
        }

        let chunks = self.chunk_notes(&key_notes);
        self.note_bundles.insert(id, GpuNoteBundle { chunks });
    }

    /// Remove previously uploaded note data by its ID.
    pub fn remove_note_data(&mut self, id: usize) {
        self.note_bundles.remove(&id);
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
        note_data_id: Option<usize>,
        style: &RenderStyle,
        clear_background: bool,
    ) {
        profile_scope!("render");
        let mode: u32 = match style.render_mode {
            RenderMode::TimeBased => 0,
            RenderMode::TickBased => 1,
        };
        let tpb = midi.and_then(|m| m.ticks_per_beat()).unwrap_or(480) as f32;
        let scroll_tick = midi
            .and_then(|m| m.tick_at_time(time))
            .unwrap_or(time * tpb as f64 * 2.0) as f32;

        let base_uniforms = ComputeUniforms {
            time: time as f32,
            scroll_tick,
            width: width as f32,
            height: height as f32,
            speed,
            keyboard_height: style.keyboard_height,
            border_width: style.border_width,
            rounding: style.rounding,
            mode,
            ticks_per_beat: tpb,
            equal_key_width: if style.equal_key_width { 1 } else { 0 },
            key_offset: 0,
            key_count: 0,
        };

        // Write per-chunk uniforms
        if let Some(bundle) = note_data_id.and_then(|id| self.note_bundles.get(&id)) {
            for chunk in &bundle.chunks {
                let u = ComputeUniforms {
                    key_offset: chunk.key_offset,
                    key_count: chunk.key_count,
                    ..base_uniforms
                };
                self.queue
                    .write_buffer(&chunk.uniform_buf, 0, bytemuck::bytes_of(&u));
            }
        }

        self.update_palette(&style.palette);

        if let Some(midi) = midi {
            self.upload_scans(
                midi,
                render_state,
                time,
                scroll_tick as f64,
                style.render_mode,
            );
        }

        let draw_keyboard = style.keyboard_height > 0.0 && midi.is_some();
        let keyboard_changed = self.is_keyboard_state_changed(
            draw_keyboard,
            time,
            scroll_tick as f64,
            style.keyboard_height,
            width,
            style.equal_key_width,
        );

        self.update_key_layouts(width, style.equal_key_width);
        self.write_render_uniforms(time, width, height);

        // Reset counter before compute
        encoder.clear_buffer(&self.compute.counter_buffer, 0, Some(4));

        let has_notes = self.dispatch_compute_pass(encoder, note_data_id);

        if keyboard_changed {
            let instances = keyboard::build_keyboard_instances(
                width,
                height,
                time,
                midi.unwrap(),
                style.keyboard_height,
                style.equal_key_width,
                &style.palette,
                render_state,
            );
            self.queue.write_buffer(
                &self.compute.keyboard_buffer,
                0,
                bytemuck::cast_slice(&instances),
            );
            self.keyboard_dirty = false;
            self.cached_keyboard_time = time;
            self.cached_scroll_tick = scroll_tick as f64;
            self.cached_keyboard_height = style.keyboard_height;
        }

        self.execute_render_pass(
            encoder,
            target,
            has_notes,
            draw_keyboard,
            style.background,
            clear_background,
        );

        self.timer.resolve(encoder);
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

    // ── Private helpers ────────────────────────────────────────────────────

    fn is_keyboard_state_changed(
        &self,
        draw_keyboard: bool,
        time: f64,
        scroll_tick: f64,
        keyboard_height: f32,
        width: u32,
        equal_key_width: bool,
    ) -> bool {
        draw_keyboard
            && (self.keyboard_dirty
                || (time - self.cached_keyboard_time).abs() > f64::EPSILON
                || (scroll_tick - self.cached_scroll_tick).abs() > f64::EPSILON
                || (keyboard_height - self.cached_keyboard_height).abs() > f32::EPSILON
                || width != self.current_width
                || equal_key_width != self.current_equal_key_width)
    }

    fn update_shared_key_layouts(queue: &Queue, buf: &Buffer, width: u32, equal_key_width: bool) {
        let layouts = keyboard::compute_key_layouts(width, equal_key_width);
        let layout_data: Vec<f32> = layouts.iter().flat_map(|(x, w)| [*x, *w]).collect();
        queue.write_buffer(buf, 0, bytemuck::cast_slice(&layout_data));
    }
}
