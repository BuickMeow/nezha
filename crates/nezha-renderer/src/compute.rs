use wgpu::*;

/// Note data stored on the GPU for compute shader consumption.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuNote {
    pub(crate) start: f32,
    pub(crate) end: f32,
    pub(crate) start_tick: u32,
    pub(crate) end_tick: u32,
    pub(crate) track: u32,
    pub(crate) velocity: u32,
}

/// Uniforms passed to the compute shader.
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

/// Per-key metadata for a compute chunk.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct KeyInfo {
    pub(crate) offset: u32,
    pub(crate) count: u32,
    pub(crate) slot: u32,
}

/// 134MB / 48bytes ≈ 2.79M, rounded down to 2.7M for 128MB limit headroom.
pub(crate) const MAX_INSTANCE_COUNT: usize = 2_700_000;

/// A single chunk of notes dispatched to the compute shader.
pub(crate) struct GpuNoteChunk {
    #[allow(dead_code)]
    pub(crate) key_info_buf: Buffer,
    #[allow(dead_code)]
    pub(crate) notes_buf: Buffer,
    pub(crate) uniform_buf: Buffer,
    pub(crate) bind_group: BindGroup,
    pub(crate) counter_buffer: Buffer,
    pub(crate) indirect_draw_buffer: Buffer,
    pub(crate) finalize_bind_group: BindGroup,
    pub(crate) key_offset: u32,
    pub(crate) key_count: u32,
}

/// All chunks belonging to one MIDI source.
pub(crate) struct GpuNoteBundle {
    pub(crate) chunks: Vec<GpuNoteChunk>,
}
