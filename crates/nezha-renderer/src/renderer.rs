use std::collections::HashMap;
use wgpu::*;

use crate::compute::{
    ComputeUniforms, GpuNote, GpuNoteBundle, GpuNoteChunk, KeyInfo, MAX_INSTANCE_COUNT,
};
use crate::gpu_timer::GpuTimer;
use crate::keyboard;
use crate::pipeline::{ComputePipelineState, RenderPipelineState};
use crate::state::MidiRenderState;
use crate::style::{NoteSource, RenderMode, RenderStyle};
use crate::vertex::{NoteInstance, Uniforms};

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

/// Maximum GPU note buffer size per chunk (120 MiB).
const MAX_NOTE_BUFFER_BYTES: u64 = 120 * 1024 * 1024;

pub struct Renderer {
    pub device: Device,
    pub queue: Queue,
    pub render: RenderPipelineState,
    pub compute: ComputePipelineState,
    pub timer: GpuTimer,

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
            MAX_INSTANCE_COUNT as u64 * instance_size,
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

        let chunks = Self::chunk_notes(&self.device, &self.queue, &self.compute, &key_notes);
        self.note_bundles.insert(id, GpuNoteBundle { chunks });
    }

    /// Remove previously uploaded note data by its ID.
    pub fn remove_note_data(&mut self, id: usize) {
        self.note_bundles.remove(&id);
    }

    /// Render one frame.
    ///
    /// * `encoder` — command encoder to record into.
    /// * `target` — texture view to render into.
    /// * `width` / `height` — viewport size in pixels.
    /// * `time` — current playback time in seconds.
    /// * `speed` — vertical scroll speed.
    /// * `midi` — optional note source for live keyboard highlights.
    /// * `render_state` — mutable scan state for fast forward / rewind.
    /// * `note_data_id` — ID returned by [`upload_note_data`](Self::upload_note_data).
    /// * `style` — visual style configuration.
    /// * `clear_background` — whether to clear the target before drawing.
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

        // Write per-chunk uniforms — scoped so the bundle borrow ends before
        // we need &mut self for other updates.
        {
            let bundle = note_data_id.and_then(|id| self.note_bundles.get(&id));
            if let Some(bundle) = bundle {
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

        // Evaluate keyboard dirty state *before* update_key_layouts mutates current_width
        let draw_keyboard = style.keyboard_height > 0.0 && midi.is_some();
        let keyboard_changed = draw_keyboard
            && (self.keyboard_dirty
                || (time - self.cached_keyboard_time).abs() > f64::EPSILON
                || ((scroll_tick as f64) - self.cached_scroll_tick).abs() > f64::EPSILON
                || (style.keyboard_height - self.cached_keyboard_height).abs() > f32::EPSILON
                || width != self.current_width
                || style.equal_key_width != self.current_equal_key_width);

        self.update_key_layouts(width, style.equal_key_width);
        self.write_render_uniforms(time, width, height);

        // Reset counter before compute
        encoder.clear_buffer(&self.compute.counter_buffer, 0, Some(4));

        // Dispatch compute — scoped so the bundle borrow ends before the
        // keyboard block, which needs &mut self.
        let has_notes = {
            let bundle = note_data_id.and_then(|id| self.note_bundles.get(&id));
            match bundle {
                Some(b) if !b.chunks.is_empty() => {
                    profile_scope!("compute_pass");
                    let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                        label: Some("compute_notes_pass"),
                        timestamp_writes: self.timer.query_set.as_ref().map(|qs| {
                            ComputePassTimestampWrites {
                                query_set: qs,
                                beginning_of_pass_write_index: Some(0),
                                end_of_pass_write_index: Some(1),
                            }
                        }),
                    });
                    cpass.set_pipeline(&self.compute.pipeline);
                    for chunk in &b.chunks {
                        cpass.set_bind_group(0, &chunk.bind_group, &[]);
                        // workgroup_size(64): ceil(key_count / 64) workgroups
                        cpass.dispatch_workgroups((chunk.key_count + 63) / 64, 1, 1);
                    }
                    drop(cpass);

                    // Copy counter → indirect draw instance_count (offset 4)
                    encoder.copy_buffer_to_buffer(
                        &self.compute.counter_buffer,
                        0,
                        &self.compute.indirect_draw_buffer,
                        4,
                        4,
                    );
                    true
                }
                _ => false,
            }
        };

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

        // (encoder is submitted by caller — no queue.submit here)

        // ── Resolve GPU timestamps ──────────────────────────────────────────
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

    fn update_shared_key_layouts(queue: &Queue, buf: &Buffer, width: u32, equal_key_width: bool) {
        let layouts = keyboard::compute_key_layouts(width, equal_key_width);
        let layout_data: Vec<f32> = layouts.iter().flat_map(|(x, w)| [*x, *w]).collect();
        queue.write_buffer(buf, 0, bytemuck::cast_slice(&layout_data));
    }

    /// Greedy chunking: group contiguous keys until the note buffer nears limit.
    fn chunk_notes(
        device: &Device,
        queue: &Queue,
        compute: &ComputePipelineState,
        key_notes: &[Vec<GpuNote>; 128],
    ) -> Vec<GpuNoteChunk> {
        let note_size = std::mem::size_of::<GpuNote>() as u64;
        let mut chunks = Vec::new();
        let mut chunk_start: u32 = 0;

        while chunk_start < 128 {
            let mut chunk_notes: Vec<GpuNote> = Vec::new();
            let mut chunk_end = chunk_start;

            // Accumulate keys until the next key would exceed the buffer limit
            while chunk_end < 128 {
                let next_len = key_notes[chunk_end as usize].len();
                let projected = (chunk_notes.len() + next_len) as u64 * note_size;
                if !chunk_notes.is_empty() && projected > MAX_NOTE_BUFFER_BYTES {
                    break;
                }
                chunk_notes.extend_from_slice(&key_notes[chunk_end as usize]);
                chunk_end += 1;
            }
            // Safety: if a single key's notes exceed the limit, we still include it
            if chunk_end == chunk_start {
                chunk_notes.extend_from_slice(&key_notes[chunk_end as usize]);
                chunk_end += 1;
            }

            let key_count = chunk_end - chunk_start;
            let chunk_info = Self::build_chunk_info(chunk_start, chunk_end, key_notes);

            let uniform_buf = device.create_buffer(&BufferDescriptor {
                label: Some("chunk_uniforms"),
                size: std::mem::size_of::<ComputeUniforms>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let key_info_buf = device.create_buffer(&BufferDescriptor {
                label: Some("key_info"),
                size: (128 * std::mem::size_of::<KeyInfo>()) as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&key_info_buf, 0, bytemuck::bytes_of(&chunk_info));

            let notes_buf = device.create_buffer(&BufferDescriptor {
                label: Some("notes"),
                size: (chunk_notes.len() * std::mem::size_of::<GpuNote>()) as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&notes_buf, 0, bytemuck::cast_slice(&chunk_notes));

            let bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("compute_bind_group"),
                layout: &compute.bgl,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: compute.shared_key_layouts_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: key_info_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: notes_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: compute.palette_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 5,
                        resource: compute.instance_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 6,
                        resource: compute.counter_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 7,
                        resource: compute.scan_buffer.as_entire_binding(),
                    },
                ],
            });

            chunks.push(GpuNoteChunk {
                key_info_buf,
                notes_buf,
                uniform_buf,
                bind_group,
                key_offset: chunk_start,
                key_count,
            });

            chunk_start = chunk_end;
        }

        chunks
    }

    fn build_chunk_info(
        chunk_start: u32,
        chunk_end: u32,
        key_notes: &[Vec<GpuNote>; 128],
    ) -> [KeyInfo; 128] {
        let mut info = [KeyInfo {
            offset: 0,
            count: 0,
            slot: 0,
        }; 128];
        let mut note_offset: u32 = 0;
        let mut white_idx = 0u32;
        let mut black_idx = 75u32;

        for key in chunk_start..chunk_end {
            let n = key_notes[key as usize].len() as u32;
            let slot = if keyboard::is_black_key(key as u8) {
                let s = black_idx;
                black_idx += 1;
                s
            } else {
                let s = white_idx;
                white_idx += 1;
                s
            };
            info[key as usize] = KeyInfo {
                offset: note_offset,
                count: n,
                slot,
            };
            note_offset += n;
        }

        info
    }

    fn update_palette(&mut self, palette: &[[f32; 3]; 128]) {
        if *palette != self.cached_palette {
            let palette_flat: Vec<f32> = palette
                .iter()
                .flat_map(|c| [c[0], c[1], c[2], 0.0f32])
                .collect();
            self.queue.write_buffer(
                &self.compute.palette_buffer,
                0,
                bytemuck::cast_slice(&palette_flat),
            );
            self.cached_palette = *palette;
        }
    }

    fn upload_scans(
        &mut self,
        midi: &dyn NoteSource,
        state: &mut MidiRenderState,
        time: f64,
        scroll_tick: f64,
        mode: RenderMode,
    ) {
        profile_scope!("scans");
        Self::advance_scan_indices(midi, state, time, scroll_tick, mode);
        let scans_u32: [u32; 128] = std::array::from_fn(|i| state.scan_indices[i] as u32);
        self.queue
            .write_buffer(&self.compute.scan_buffer, 0, bytemuck::bytes_of(&scans_u32));
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

    fn update_key_layouts(&mut self, width: u32, equal_key_width: bool) {
        if width != self.current_width || equal_key_width != self.current_equal_key_width {
            Self::update_shared_key_layouts(
                &self.queue,
                &self.compute.shared_key_layouts_buf,
                width,
                equal_key_width,
            );
            self.current_width = width;
            self.current_equal_key_width = equal_key_width;
        }
    }

    fn write_render_uniforms(&mut self, time: f64, width: u32, height: u32) {
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
    }

    fn execute_render_pass(
        &self,
        encoder: &mut CommandEncoder,
        target: &TextureView,
        has_notes: bool,
        draw_keyboard: bool,
        background: [f64; 4],
        clear_background: bool,
    ) {
        profile_scope!("render_pass");

        let load_op = if clear_background {
            LoadOp::Clear(Color {
                r: background[0],
                g: background[1],
                b: background[2],
                a: background[3],
            })
        } else {
            LoadOp::Load
        };

        let mut pass =
            encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("render_pass"),
                timestamp_writes: self.timer.query_set.as_ref().map(|qs| {
                    RenderPassTimestampWrites {
                        query_set: qs,
                        beginning_of_pass_write_index: Some(2),
                        end_of_pass_write_index: Some(3),
                    }
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

        if has_notes {
            pass.set_pipeline(&self.render.pipeline);
            pass.set_bind_group(0, &self.render.bind_group, &[]);
            pass.set_vertex_buffer(0, self.compute.instance_buffer.slice(..));
            pass.draw_indirect(&self.compute.indirect_draw_buffer, 0);
        }

        if draw_keyboard {
            pass.set_pipeline(&self.render.pipeline);
            pass.set_bind_group(0, &self.render.bind_group, &[]);
            pass.set_vertex_buffer(0, self.compute.keyboard_buffer.slice(..));
            pass.draw(0..6, 0..75); // white keys (slots 0-74)
            pass.draw(0..6, 75..128); // black keys (slots 75-127)
        }
    }
}
