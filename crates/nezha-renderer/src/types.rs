use std::collections::HashMap;
use wgpu::*;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Uniforms {
    pub(crate) time: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NoteInstance {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    pub corner_radius: f32,
    pub border_width: f32,
    pub _pad: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuNote {
    pub(crate) start: f32,
    pub(crate) end: f32,
    pub(crate) start_tick: u32,
    pub(crate) end_tick: u32,
    pub(crate) track: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ComputeUniforms {
    pub(crate) time: f32,
    pub(crate) scroll_tick: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) speed: f32,
    pub(crate) keyboard_height: f32,
    pub(crate) border_width: f32,
    pub(crate) rounding: f32,
    pub(crate) mode: u32,
    pub(crate) ticks_per_beat: f32,
    pub(crate) equal_key_width: u32,
    pub(crate) key_offset: u32,
    pub(crate) key_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct KeyInfo {
    pub(crate) offset: u32,
    pub(crate) count: u32,
    pub(crate) slot: u32,
}

/// 134MB / 48bytes ≈ 2.79M，取整 2.7M 留余量兼容 128MB 限制
pub(crate) const MAX_INSTANCE_COUNT: usize = 2_700_000;

pub(crate) struct GpuNoteChunk {
    #[allow(dead_code)]
    pub(crate) key_info_buf: Buffer,
    #[allow(dead_code)]
    pub(crate) notes_buf: Buffer,
    pub(crate) uniform_buf: Buffer,
    pub(crate) bind_group: BindGroup,
    pub(crate) key_offset: u32,
    pub(crate) key_count: u32,
}

pub(crate) struct GpuNoteBundle {
    pub(crate) chunks: Vec<GpuNoteChunk>,
}

pub struct Renderer {
    pub device: Device,
    pub queue: Queue,
    pub(crate) pipeline: RenderPipeline,
    pub(crate) uniform_buffer: Buffer,
    pub(crate) render_bind_group: BindGroup,

    pub(crate) compute_pipeline: ComputePipeline,
    pub(crate) shared_key_layouts_buf: Buffer,
    pub(crate) scan_buffer: Buffer,
    pub(crate) compute_bgl: BindGroupLayout,
    pub(crate) palette_buffer: Buffer,
    pub(crate) instance_buffer: Buffer,
    pub(crate) keyboard_buffer: Buffer,
    pub(crate) counter_buffer: Buffer,
    pub(crate) indirect_draw_buffer: Buffer,

    pub(crate) note_bundles: HashMap<usize, GpuNoteBundle>,
    pub(crate) current_width: u32,
    pub(crate) current_equal_key_width: bool,
    pub(crate) cached_palette: [[f32; 3]; 128],
    /// Keyboard dirty flag — skips CPU recomputation when time/style hasn't changed
    pub(crate) keyboard_dirty: bool,
    pub(crate) cached_keyboard_time: f64,
    pub(crate) cached_scroll_tick: f64,
    pub(crate) cached_keyboard_height: f32,
}
