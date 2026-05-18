use wgpu::*;

use crate::gpu_timer::GpuTimer;
use crate::pipeline::ComputePipelineState;
use crate::pipeline::RenderPipelineState;
use crate::source::NoteSource;
use crate::state::MidiRenderState;
use crate::style::RenderStyle;
use crate::vertex::NoteInstance;

use cache::RendererCache;
use frame::PreparedFrame;

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
    cache: RendererCache,
}

mod cache;
mod chunk;
mod frame;
mod pass;
mod scan;
mod upload;

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
            cache: RendererCache::default(),
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
        note_data_id: Option<usize>,
        style: &RenderStyle,
        clear_background: bool,
    ) {
        profile_scope!("render");
        let PreparedFrame {
            scroll_tick,
            base_uniforms,
            draw_keyboard,
            keyboard_changed,
        } = self.prepare_frame(width, height, time, speed, midi, style);

        self.write_chunk_uniforms(note_data_id, base_uniforms);

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

        self.update_key_layouts(width, style.equal_key_width);
        self.write_render_uniforms(time, width, height);

        // Reset counter before compute
        encoder.clear_buffer(&self.compute.counter_buffer, 0, Some(4));

        let has_notes = self.dispatch_compute_pass(encoder, note_data_id);

        if keyboard_changed {
            if let Some(midi) = midi {
                self.update_keyboard_instances(
                    width,
                    height,
                    time,
                    scroll_tick as f64,
                    midi,
                    style,
                    render_state,
                );
            }
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
}
