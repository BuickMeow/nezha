use wgpu::*;

use rayon::prelude::*;
use std::collections::HashMap;

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

#[derive(Default)]
struct KeySeekIndex {
    block_prefix_max_end: Vec<f64>,
    block_prefix_max_end_tick: Vec<u32>,
}

impl KeySeekIndex {
    fn build(notes: &[nezha_core::Note]) -> Self {
        let mut block_prefix_max_end =
            Vec::with_capacity(notes.len().div_ceil(SEEK_INDEX_BLOCK_SIZE));
        let mut block_prefix_max_end_tick =
            Vec::with_capacity(notes.len().div_ceil(SEEK_INDEX_BLOCK_SIZE));
        let mut max_end = f64::NEG_INFINITY;
        let mut max_end_tick = 0u32;

        for block in notes.chunks(SEEK_INDEX_BLOCK_SIZE) {
            for note in block {
                max_end = max_end.max(note.end);
                max_end_tick = max_end_tick.max(note.end_tick);
            }
            block_prefix_max_end.push(max_end);
            block_prefix_max_end_tick.push(max_end_tick);
        }

        Self {
            block_prefix_max_end,
            block_prefix_max_end_tick,
        }
    }

    fn scan_index_for_time(&self, notes: &[nezha_core::Note], time: f64) -> usize {
        if notes.is_empty() {
            return 0;
        }
        let completed_blocks = self
            .block_prefix_max_end
            .partition_point(|&prefix_max_end| prefix_max_end <= time);
        let mut scan = completed_blocks
            .saturating_mul(SEEK_INDEX_BLOCK_SIZE)
            .min(notes.len());
        let local_end = (scan + SEEK_INDEX_BLOCK_SIZE).min(notes.len());
        while scan < local_end && notes[scan].end <= time {
            scan += 1;
        }
        scan
    }

    fn scan_index_for_tick(&self, notes: &[nezha_core::Note], scroll_tick: f64) -> usize {
        if notes.is_empty() {
            return 0;
        }
        let completed_blocks = self
            .block_prefix_max_end_tick
            .partition_point(|&prefix_max_end_tick| (prefix_max_end_tick as f64) <= scroll_tick);
        let mut scan = completed_blocks
            .saturating_mul(SEEK_INDEX_BLOCK_SIZE)
            .min(notes.len());
        let local_end = (scan + SEEK_INDEX_BLOCK_SIZE).min(notes.len());
        while scan < local_end && (notes[scan].end_tick as f64) <= scroll_tick {
            scan += 1;
        }
        scan
    }
}

struct NoteSeekIndex {
    per_key: [KeySeekIndex; 128],
}

impl NoteSeekIndex {
    fn build(source: &dyn NoteSource) -> Self {
        Self {
            per_key: std::array::from_fn(|key| KeySeekIndex::build(source.key_notes(key as u8))),
        }
    }
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
    note_seek_indices: HashMap<usize, NoteSeekIndex>,
    current_batch_counts: Vec<usize>,
}

/// CPU path keeps multi-buffer batching to avoid a single hard cap per frame.
const MAX_INSTANCE_COUNT: usize = 6_000_000;
const MAX_PARALLEL_KEY_GROUPS: usize = 16;
const SEEK_INDEX_BLOCK_SIZE: usize = 256;
const MIN_INSTANCE_BUFFER_CAPACITY: usize = 4_096;

struct InstanceBufferSlot {
    buffer: Buffer,
    capacity_instances: usize,
}

struct KeyChunkBuildResult {
    instances: Vec<NoteInstance>,
    active_keys: [bool; 128],
    active_colors: [[f32; 3]; 128],
}

impl KeyChunkBuildResult {
    fn new() -> Self {
        Self {
            instances: Vec::new(),
            active_keys: [false; 128],
            active_colors: [[0.0; 3]; 128],
        }
    }
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
            note_seek_indices: HashMap::new(),
            current_batch_counts: Vec::new(),
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
        render_state: &mut MidiRenderState,
        note_data_id: Option<usize>,
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

        match midi {
            Some(m) => Self::build_instances(
                &mut instances,
                layouts,
                height,
                time,
                speed,
                m,
                render_state,
                note_data_id.and_then(|id| self.note_seek_indices.get(&id)),
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
            }
        }

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
                .push(Self::create_instance_buffer_slot(
                    &self.device,
                    instance_size,
                    MIN_INSTANCE_BUFFER_CAPACITY,
                ));
        }
        for (i, batch) in batches.iter().enumerate() {
            let required_instances = batch.len().max(1);
            if self.instance_buffers[i].capacity_instances < required_instances {
                self.instance_buffers[i] = Self::create_instance_buffer_slot(
                    &self.device,
                    instance_size,
                    Self::next_instance_capacity(required_instances),
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
        load_op: wgpu::LoadOp<wgpu::Color>,
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
        self.prepare(
            width,
            height,
            time,
            speed,
            midi,
            render_state,
            _note_data_id,
            style,
        );

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

        self.draw(encoder, target, load_op);
        self.timer.resolve(encoder);
    }

    pub fn upload_note_data(
        &mut self,
        id: usize,
        source: &dyn NoteSource,
        _width: u32,
        _equal_key_width: bool,
    ) {
        profile_scope!("upload_note_data");
        self.note_seek_indices
            .entry(id)
            .or_insert_with(|| NoteSeekIndex::build(source));
    }

    pub fn remove_note_data(&mut self, id: usize) {
        self.note_seek_indices.remove(&id);
    }

    pub fn clear_note_data(&mut self) {
        self.note_seek_indices.clear();
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

    fn build_instances(
        instances: &mut Vec<NoteInstance>,
        layouts: &[(f32, f32)],
        height: u32,
        time: f64,
        speed: f32,
        midi: &dyn NoteSource,
        state: &mut MidiRenderState,
        seek_index: Option<&NoteSeekIndex>,
        style: &RenderStyle,
    ) {
        let mut active_keys = [false; 128];
        let mut active_colors = [[0.0f32; 3]; 128];

        let scroll_tick = Self::scroll_tick_for_mode(midi, time, style);
        Self::advance_scan_indices(
            midi,
            state,
            time,
            scroll_tick,
            style.render_mode,
            seek_index,
        );
        let scan_indices = state.scan_indices;
        let render_keys = Self::build_render_key_order(style.equal_key_width);

        match style.render_mode {
            RenderMode::TimeBased => Self::build_instances_time(
                instances,
                layouts,
                &render_keys,
                &scan_indices,
                &mut active_keys,
                &mut active_colors,
                height,
                time,
                speed,
                midi,
                style,
            ),
            RenderMode::TickBased => Self::build_instances_tick(
                instances,
                layouts,
                &render_keys,
                &scan_indices,
                &mut active_keys,
                &mut active_colors,
                height,
                time,
                speed,
                midi,
                style,
            ),
        };

        if style.keyboard_height > 0.0 {
            keyboard::append_keyboard_instances(
                layouts,
                height,
                style.keyboard_height,
                &active_keys,
                &active_colors,
                instances,
            );
        }
    }

    fn build_instances_time(
        instances: &mut Vec<NoteInstance>,
        layouts: &[(f32, f32)],
        render_keys: &[u8; 128],
        scan_indices: &[usize; 128],
        active_keys: &mut [bool; 128],
        active_colors: &mut [[f32; 3]; 128],
        height: u32,
        time: f64,
        speed: f32,
        midi: &dyn NoteSource,
        style: &RenderStyle,
    ) {
        let kh = (style.keyboard_height as f64).max(0.0);
        let effective_h = (height as f64 - kh).max(1.0);
        let pps = 200.0f64 * speed.max(0.01) as f64;
        let screen_top = effective_h + time * pps;
        let time_top = time + effective_h / pps + 1.0;
        let time_bottom = time;
        let key_groups = Self::build_parallel_key_groups(render_keys, scan_indices, midi);
        let chunk_results = key_groups
            .into_par_iter()
            .map(|range| {
                let mut result = KeyChunkBuildResult::new();
                for &key in &render_keys[range] {
                    Self::append_key_instances_time(
                        &mut result,
                        key,
                        layouts,
                        scan_indices[key as usize],
                        time,
                        time_top,
                        time_bottom,
                        screen_top,
                        pps,
                        midi,
                        style,
                    );
                }
                result
            })
            .collect::<Vec<_>>();

        for chunk in chunk_results {
            for key in 0..128usize {
                if chunk.active_keys[key] {
                    active_keys[key] = true;
                    active_colors[key] = chunk.active_colors[key];
                }
            }
            instances.extend(chunk.instances);
        }
    }

    fn build_instances_tick(
        instances: &mut Vec<NoteInstance>,
        layouts: &[(f32, f32)],
        render_keys: &[u8; 128],
        scan_indices: &[usize; 128],
        active_keys: &mut [bool; 128],
        active_colors: &mut [[f32; 3]; 128],
        height: u32,
        time: f64,
        speed: f32,
        midi: &dyn NoteSource,
        style: &RenderStyle,
    ) {
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

        let key_groups = Self::build_parallel_key_groups(render_keys, scan_indices, midi);
        let chunk_results = key_groups
            .into_par_iter()
            .map(|range| {
                let mut result = KeyChunkBuildResult::new();
                for &key in &render_keys[range] {
                    Self::append_key_instances_tick(
                        &mut result,
                        key,
                        layouts,
                        scan_indices[key as usize],
                        time,
                        tick_at_top,
                        scroll_tick,
                        screen_bottom,
                        ppt,
                        midi,
                        style,
                    );
                }
                result
            })
            .collect::<Vec<_>>();

        for chunk in chunk_results {
            for key in 0..128usize {
                if chunk.active_keys[key] {
                    active_keys[key] = true;
                    active_colors[key] = chunk.active_colors[key];
                }
            }
            instances.extend(chunk.instances);
        }
    }

    fn advance_scan_indices(
        midi: &dyn NoteSource,
        state: &mut MidiRenderState,
        time: f64,
        scroll_tick: f64,
        mode: RenderMode,
        seek_index: Option<&NoteSeekIndex>,
    ) {
        match mode {
            RenderMode::TimeBased => {
                let rewound = time < state.last_time;
                state.last_time = time;
                if let Some(seek_index) = seek_index {
                    state
                        .scan_indices
                        .par_iter_mut()
                        .enumerate()
                        .for_each(|(key, scan_slot)| {
                            let notes = midi.key_notes(key as u8);
                            *scan_slot = seek_index.per_key[key].scan_index_for_time(notes, time);
                        });
                } else {
                    if rewound {
                        state.scan_indices = [0; 128];
                    }
                    state
                        .scan_indices
                        .par_iter_mut()
                        .enumerate()
                        .for_each(|(key, scan_slot)| {
                            let notes = midi.key_notes(key as u8);
                            if notes.is_empty() {
                                *scan_slot = 0;
                                return;
                            }

                            let mut scan = (*scan_slot).min(notes.len());
                            while scan < notes.len() && notes[scan].end <= time {
                                scan += 1;
                            }
                            *scan_slot = scan;
                        });
                }
            }
            RenderMode::TickBased => {
                let rewound = scroll_tick < state.last_scroll_tick;
                state.last_scroll_tick = scroll_tick;
                if let Some(seek_index) = seek_index {
                    state
                        .scan_indices
                        .par_iter_mut()
                        .enumerate()
                        .for_each(|(key, scan_slot)| {
                            let notes = midi.key_notes(key as u8);
                            *scan_slot =
                                seek_index.per_key[key].scan_index_for_tick(notes, scroll_tick);
                        });
                } else {
                    if rewound {
                        state.scan_indices = [0; 128];
                    }
                    state
                        .scan_indices
                        .par_iter_mut()
                        .enumerate()
                        .for_each(|(key, scan_slot)| {
                            let notes = midi.key_notes(key as u8);
                            if notes.is_empty() {
                                *scan_slot = 0;
                                return;
                            }

                            let mut scan = (*scan_slot).min(notes.len());
                            while scan < notes.len() && (notes[scan].end_tick as f64) <= scroll_tick
                            {
                                scan += 1;
                            }
                            *scan_slot = scan;
                        });
                }
            }
        }
    }

    fn scroll_tick_for_mode(midi: &dyn NoteSource, time: f64, style: &RenderStyle) -> f64 {
        match style.render_mode {
            RenderMode::TimeBased => -1.0,
            RenderMode::TickBased => {
                let ticks_per_beat = midi.ticks_per_beat().unwrap_or(480) as f64;
                midi.tick_at_time(time)
                    .unwrap_or(time * ticks_per_beat * 2.0)
            }
        }
    }

    fn build_render_key_order(equal_key_width: bool) -> [u8; 128] {
        let mut keys = [0u8; 128];
        if equal_key_width {
            for key in 0..128u8 {
                keys[key as usize] = key;
            }
        } else {
            let mut idx = 0usize;
            for key in 0..128u8 {
                if !keyboard::is_black_key(key) {
                    keys[idx] = key;
                    idx += 1;
                }
            }
            for key in 0..128u8 {
                if keyboard::is_black_key(key) {
                    keys[idx] = key;
                    idx += 1;
                }
            }
        }
        keys
    }

    fn build_parallel_key_groups(
        render_keys: &[u8; 128],
        scan_indices: &[usize; 128],
        midi: &dyn NoteSource,
    ) -> Vec<std::ops::Range<usize>> {
        let mut total_weight = 0usize;
        let mut active_key_count = 0usize;
        let mut weights = [0usize; 128];
        for (i, &key) in render_keys.iter().enumerate() {
            let notes = midi.key_notes(key);
            let remaining = notes.len().saturating_sub(scan_indices[key as usize]);
            let weight = remaining.max(1);
            weights[i] = weight;
            total_weight += weight;
            if !notes.is_empty() {
                active_key_count += 1;
            }
        }

        if active_key_count <= 1 {
            return vec![0..128];
        }

        let thread_budget = rayon::current_num_threads().max(1);
        let desired_groups = if total_weight < 8_192 {
            thread_budget
        } else {
            thread_budget.saturating_mul(2)
        }
        .min(MAX_PARALLEL_KEY_GROUPS)
        .min(active_key_count)
        .max(1);

        let target_weight = total_weight.div_ceil(desired_groups);
        let mut ranges = Vec::with_capacity(desired_groups);
        let mut start = 0usize;
        let mut acc = 0usize;

        for i in 0..128usize {
            let remaining_keys = 128usize - i;
            let remaining_groups = desired_groups.saturating_sub(ranges.len());
            if remaining_groups == 0 {
                break;
            }

            acc += weights[i];
            let should_split = acc >= target_weight && remaining_keys > remaining_groups;
            if should_split {
                ranges.push(start..(i + 1));
                start = i + 1;
                acc = 0;
            }
        }

        if start < 128 {
            ranges.push(start..128);
        }
        if ranges.is_empty() {
            ranges.push(0..128);
        }
        ranges
    }

    fn next_instance_capacity(required_instances: usize) -> usize {
        required_instances
            .max(MIN_INSTANCE_BUFFER_CAPACITY)
            .next_power_of_two()
            .min(MAX_INSTANCE_COUNT)
    }

    fn create_instance_buffer_slot(
        device: &Device,
        instance_size: u64,
        capacity_instances: usize,
    ) -> InstanceBufferSlot {
        InstanceBufferSlot {
            buffer: device.create_buffer(&BufferDescriptor {
                label: Some("instance_buffer"),
                size: capacity_instances as u64 * instance_size,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity_instances,
        }
    }

    fn append_key_instances_time(
        result: &mut KeyChunkBuildResult,
        key: u8,
        layouts: &[(f32, f32)],
        scan: usize,
        time: f64,
        time_top: f64,
        time_bottom: f64,
        screen_top: f64,
        pps: f64,
        midi: &dyn NoteSource,
        style: &RenderStyle,
    ) {
        let notes = midi.key_notes(key);
        if notes.is_empty() {
            return;
        }
        let (x, w) = layouts[key as usize];

        for note in &notes[scan.min(notes.len())..] {
            if note.start > time_top {
                break;
            }
            if note.end <= time_bottom {
                continue;
            }

            let trk = note.track as usize % 128;
            let [r, g, b] = style.palette[trk];
            if note.start <= time && time < note.end {
                result.active_keys[key as usize] = true;
                result.active_colors[key as usize] = [r, g, b];
            }

            let note_bottom = (screen_top - note.start * pps) as f32;
            let note_top = (screen_top - note.end * pps) as f32;
            let h = (note_bottom - note_top).max(1.0);
            result.instances.push(NoteInstance {
                x,
                y: note_top,
                w,
                h,
                rgba_packed: pack_rgba(r, g, b, 1.0),
                props_packed: pack_props(
                    style.rounding * f32::min(w, h),
                    style.border_width * w / 2.0,
                ),
                velocity: note.velocity as u32,
                flags: 0,
            });
        }
    }

    fn append_key_instances_tick(
        result: &mut KeyChunkBuildResult,
        key: u8,
        layouts: &[(f32, f32)],
        scan: usize,
        time: f64,
        tick_at_top: f64,
        scroll_tick: f64,
        screen_bottom: f64,
        ppt: f64,
        midi: &dyn NoteSource,
        style: &RenderStyle,
    ) {
        let notes = midi.key_notes(key);
        if notes.is_empty() {
            return;
        }
        let (x, w) = layouts[key as usize];

        for note in &notes[scan.min(notes.len())..] {
            if (note.start_tick as f64) > tick_at_top + 1.0 {
                break;
            }
            if (note.end_tick as f64) <= scroll_tick {
                continue;
            }

            let trk = note.track as usize % 128;
            let [r, g, b] = style.palette[trk];
            if note.start <= time && time < note.end {
                result.active_keys[key as usize] = true;
                result.active_colors[key as usize] = [r, g, b];
            }

            let note_top = (screen_bottom - note.end_tick as f64 * ppt) as f32;
            let note_bottom = (screen_bottom - note.start_tick as f64 * ppt) as f32;
            let h = (note_bottom - note_top).max(1.0);
            result.instances.push(NoteInstance {
                x,
                y: note_top,
                w,
                h,
                rgba_packed: pack_rgba(r, g, b, 1.0),
                props_packed: pack_props(
                    style.rounding * f32::min(w, h),
                    style.border_width * w / 2.0,
                ),
                velocity: note.velocity as u32,
                flags: 0,
            });
        }
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
