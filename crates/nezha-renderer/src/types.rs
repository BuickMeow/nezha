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

/// Packed instance: 32 bytes (was 48).
/// Layout: xywh (vec4 f32) + packed (vec4 u32) = 2 vertex attributes.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct NoteInstance {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// RGBA packed as 4×UNORM8: R|G<<8|B<<16|A<<24
    pub rgba_packed: u32,
    /// corner_radius (f16 high) | border_width (f16 low)
    pub props_packed: u32,
    /// MIDI velocity 0-127 (reserved for future use)
    pub velocity: u32,
    /// Bit flags (reserved)
    pub flags: u32,
}

/// Pack RGBA floats (0.0–1.0) into a single u32 (UNORM8 × 4).
pub fn pack_rgba(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let r8 = (r.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    let g8 = (g.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    let b8 = (b.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    let a8 = (a.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    r8 | (g8 << 8) | (b8 << 16) | (a8 << 24)
}

/// Pack corner_radius and border_width (both f32) into a single u32 (2×f16).
pub fn pack_props(corner_radius: f32, border_width: f32) -> u32 {
    let cr = half::f16::from_f32(corner_radius);
    let bw = half::f16::from_f32(border_width);
    (cr.to_bits() as u32) | ((bw.to_bits() as u32) << 16)
}

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

    // ── GPU timestamp queries ──────────────────────────────────────────────
    pub(crate) gpu_timing_supported: bool,
    pub(crate) timestamp_query_set: Option<QuerySet>,
    pub(crate) timestamp_resolve_buffer: Option<Buffer>,
    pub(crate) timestamp_readback_buffer: Option<Buffer>,
    pub(crate) timestamp_period: f32,
}
