use std::collections::HashMap;
use wgpu::*;

use crate::keyboard;
use crate::style::{MidiRenderState, NoteSource, RenderMode, RenderStyle};
use crate::types::{
    ComputeUniforms, GpuNote, GpuNoteBundle, GpuNoteChunk, MAX_INSTANCE_COUNT, NoteInstance,
    Renderer, Uniforms,
};

impl Renderer {
    pub fn new(device: Device, queue: Queue, format: TextureFormat) -> Self {
        let render_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("waterfall_shader"),
            source: ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let compute_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("compute_notes"),
            source: ShaderSource::Wgsl(include_str!("compute_notes.wgsl").into()),
        });

        // ---- Render pipeline ----
        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let render_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("render_bind_group_layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let render_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("render_bind_group"),
            layout: &render_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[Some(&render_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &render_shader,
                entry_point: Some("vs_main"),
                buffers: &[VertexBufferLayout {
                    array_stride: std::mem::size_of::<NoteInstance>() as u64,
                    step_mode: VertexStepMode::Instance,
                    attributes: &vertex_attr_array![
                        0 => Float32x4,
                        1 => Float32x4,
                        2 => Float32x2,
                    ],
                }],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: &render_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                ..PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // ---- Compute pipeline ----
        let shared_key_layouts_buf = device.create_buffer(&BufferDescriptor {
            label: Some("shared_key_layouts"),
            size: (128 * 2 * 4) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let scan_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("scans"),
            size: (128 * 4) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let palette_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("palette"),
            size: (128 * 4 * 4) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instance_size = std::mem::size_of::<NoteInstance>() as u64;
        let instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("instance_output"),
            size: MAX_INSTANCE_COUNT as u64 * instance_size,
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let keyboard_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("keyboard_instances"),
            size: 256 * instance_size,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let counter_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("counter"),
            size: 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let indirect_draw_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("indirect_draw"),
            size: 16,
            usage: BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        queue.write_buffer(
            &indirect_draw_buffer,
            0,
            bytemuck::bytes_of(&[6u32, 0u32, 0u32, 0u32]),
        );

        // Compute bind group layout (shared across all note bundles)
        let compute_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("compute_bgl"),
            entries: &[
                // 0: ComputeUniforms
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 1: key_layouts
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 2: key_offsets
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 3: key_counts
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 4: notes
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 5: palette
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 6: instances (output)
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 7: instance_count (atomic)
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: Some(std::num::NonZeroU64::new(4).unwrap()),
                    },
                    count: None,
                },
                // 8: key_scans (shared, updated per frame)
                BindGroupLayoutEntry {
                    binding: 8,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let compute_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("compute_pipeline_layout"),
            bind_group_layouts: &[Some(&compute_bgl)],
            immediate_size: 0,
        });

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("compute_notes_pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("compute_notes"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        Self {
            device,
            queue,
            pipeline,
            uniform_buffer,
            render_bind_group,
            compute_pipeline,
            shared_key_layouts_buf,
            scan_buffer,
            compute_bgl,
            palette_buffer,
            instance_buffer,
            keyboard_buffer,
            counter_buffer,
            indirect_draw_buffer,
            note_bundles: HashMap::new(),
            current_width: 0,
            current_equal_key_width: false,
            cached_palette: [[0.0; 3]; 128],
        }
    }

    pub fn upload_note_data(
        &mut self,
        id: usize,
        source: &dyn NoteSource,
        width: u32,
        equal_key_width: bool,
    ) {
        Self::update_shared_key_layouts(
            &self.queue,
            &self.shared_key_layouts_buf,
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
                });
            }
        }

        // Greedy chunking: add keys to a chunk until notes buffer nears limit
        let max_note_bytes: u64 = 120 * 1024 * 1024;
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
                if !chunk_notes.is_empty() && projected > max_note_bytes {
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
            let mut chunk_offsets = [0u32; 128];
            let mut chunk_counts = [0u32; 128];
            let mut note_offset: u32 = 0;

            for key in chunk_start..chunk_end {
                chunk_offsets[key as usize] = note_offset;
                let n = key_notes[key as usize].len() as u32;
                chunk_counts[key as usize] = n;
                note_offset += n;
            }

            let uniform_buf = self.device.create_buffer(&BufferDescriptor {
                label: Some("chunk_uniforms"),
                size: std::mem::size_of::<ComputeUniforms>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let key_offsets_buf = self.device.create_buffer(&BufferDescriptor {
                label: Some("key_offsets"),
                size: (128 * 4) as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue
                .write_buffer(&key_offsets_buf, 0, bytemuck::bytes_of(&chunk_offsets));

            let key_counts_buf = self.device.create_buffer(&BufferDescriptor {
                label: Some("key_counts"),
                size: (128 * 4) as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue
                .write_buffer(&key_counts_buf, 0, bytemuck::bytes_of(&chunk_counts));

            let notes_buf = self.device.create_buffer(&BufferDescriptor {
                label: Some("notes"),
                size: (chunk_notes.len() * std::mem::size_of::<GpuNote>()) as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue
                .write_buffer(&notes_buf, 0, bytemuck::cast_slice(&chunk_notes));

            let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("compute_bind_group"),
                layout: &self.compute_bgl,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: self.shared_key_layouts_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: key_offsets_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: key_counts_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: notes_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 5,
                        resource: self.palette_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 6,
                        resource: self.instance_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 7,
                        resource: self.counter_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 8,
                        resource: self.scan_buffer.as_entire_binding(),
                    },
                ],
            });

            chunks.push(GpuNoteChunk {
                key_offsets_buf,
                key_counts_buf,
                notes_buf,
                uniform_buf,
                bind_group,
                key_offset: chunk_start,
                key_count,
            });

            chunk_start = chunk_end;
        }

        self.note_bundles.insert(id, GpuNoteBundle { chunks });
    }

    pub fn remove_note_data(&mut self, id: usize) {
        self.note_bundles.remove(&id);
    }

    fn update_shared_key_layouts(queue: &Queue, buf: &Buffer, width: u32, equal_key_width: bool) {
        let layouts = keyboard::compute_key_layouts(width, equal_key_width);
        let layout_data: Vec<f32> = layouts.iter().flat_map(|(x, w)| [*x, *w]).collect();
        queue.write_buffer(buf, 0, bytemuck::cast_slice(&layout_data));
    }

    fn update_keyboard_scans(
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

        // Write per-chunk uniforms BEFORE creating the encoder (safe ordering)
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

        // Update palette (only when changed)
        if style.palette != self.cached_palette {
            let palette_flat: Vec<f32> = style
                .palette
                .iter()
                .flat_map(|c| [c[0], c[1], c[2], 0.0f32])
                .collect();
            self.queue
                .write_buffer(&self.palette_buffer, 0, bytemuck::cast_slice(&palette_flat));
            self.cached_palette = style.palette;
        }

        // Update keyboard scans (used for both CPU keyboard and GPU compute scan skipping)
        if let Some(midi) = midi {
            Self::update_keyboard_scans(
                midi,
                render_state,
                time,
                scroll_tick as f64,
                style.render_mode,
            );
            let scans_u32: [u32; 128] =
                std::array::from_fn(|i| render_state.scan_indices[i] as u32);
            self.queue
                .write_buffer(&self.scan_buffer, 0, bytemuck::bytes_of(&scans_u32));
        }

        // Update shared key layouts if needed
        let eqw = style.equal_key_width;
        if width != self.current_width || eqw != self.current_equal_key_width {
            Self::update_shared_key_layouts(&self.queue, &self.shared_key_layouts_buf, width, eqw);
            self.current_width = width;
            self.current_equal_key_width = eqw;
        }

        // Existing render uniforms (for vertex/fragment shader)
        let uniforms = Uniforms {
            time: time as f32,
            width: width as f32,
            height: height as f32,
            _pad: 0.0,
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Reset counter before compute
        encoder.clear_buffer(&self.counter_buffer, 0, Some(4));

        let mut has_notes = false;
        if let Some(bundle) = bundle {
            if !bundle.chunks.is_empty() {
                has_notes = true;
                {
                    let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor {
                        label: Some("compute_notes_pass"),
                        timestamp_writes: None,
                    });
                    cpass.set_pipeline(&self.compute_pipeline);
                    for chunk in &bundle.chunks {
                        cpass.set_bind_group(0, &chunk.bind_group, &[]);
                        // workgroup_size(64): ceil(key_count / 64) workgroups
                        cpass.dispatch_workgroups((chunk.key_count + 63) / 64, 1, 1);
                    }
                }
                // Copy counter → indirect draw instance_count (offset 4)
                encoder.copy_buffer_to_buffer(
                    &self.counter_buffer,
                    0,
                    &self.indirect_draw_buffer,
                    4,
                    4,
                );
            }
        }

        // Keyboard instances (CPU)
        let keyboard = if style.keyboard_height > 0.0 {
            if let Some(midi) = midi {
                keyboard::build_keyboard_instances(width, height, time, midi, style, render_state)
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

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
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if has_notes {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.render_bind_group, &[]);
                pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
                pass.draw_indirect(&self.indirect_draw_buffer, 0);
            }

            if !keyboard.is_empty() {
                self.queue
                    .write_buffer(&self.keyboard_buffer, 0, bytemuck::cast_slice(&keyboard));

                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.render_bind_group, &[]);
                pass.set_vertex_buffer(0, self.keyboard_buffer.slice(..));
                pass.draw(0..6, 0..keyboard.len() as u32);
            }
        }

        // (encoder is submitted by caller — no queue.submit here)
    }
}
