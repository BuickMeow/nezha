use wgpu::*;
use nezha_core::MidiFile;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    time: f32,
    width: f32,
    height: f32,
    _pad: f32,
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
    pub _pad: f32,
}

/// 音符数据源抽象，解耦 renderer 与具体 MIDI 格式
pub trait NoteSource {
    /// 返回该 key 的所有音符（已按 start 排序）
    fn key_notes(&self, key: u8) -> &[nezha_core::Note];
    /// 总时长（秒）
    fn duration(&self) -> f64;
    /// PPQ (ticks per beat)，返回 None 表示无 tick 信息，降级为秒计算
    fn ticks_per_beat(&self) -> Option<u32> { None }
}

/// 渲染风格配置
#[derive(Clone)]
pub struct RenderStyle {
    /// 边框宽度比例 0.0~1.0（1.0 表示左边 50% + 右边 50% 都是边框）
    pub border_width: f32,
    /// 圆角比例 0.0~1.0（1.0 表示底部是完全的半圆）
    pub rounding: f32,
    /// 每个 MIDI channel 的颜色 (channel 0..15)
    pub channel_colors: [[f32; 3]; 16],
}

impl Default for RenderStyle {
    fn default() -> Self {
        Self {
            border_width: 0.1,
            rounding: 0.0,
            channel_colors: [[0.3, 0.5, 0.9]; 16],
        }
    }
}

impl NoteSource for MidiFile {
    fn key_notes(&self, key: u8) -> &[nezha_core::Note] {
        &self.key_notes[key as usize]
    }

    fn duration(&self) -> f64 {
        self.duration
    }

    fn ticks_per_beat(&self) -> Option<u32> {
        Some(self.ticks_per_beat)
    }
}

/// MIDI 渲染业务状态（与 GPU 资源分离）
pub struct MidiRenderState {
    scan_indices: [usize; 128],
    last_time: f64,
}

impl Default for MidiRenderState {
    fn default() -> Self {
        Self {
            scan_indices: [0; 128],
            last_time: -1.0,
        }
    }
}

impl MidiRenderState {
    pub fn reset(&mut self) {
        self.scan_indices = [0; 128];
        self.last_time = -1.0;
    }
}

/// GPU 资源管理 + 渲染调度
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
}

impl Renderer {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("waterfall_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<NoteInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4,
                        1 => Float32x4,
                        2 => Float32x2,
                    ],
                }],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let instance_capacity = 1024;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance_buffer"),
            size: (instance_capacity * std::mem::size_of::<NoteInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            pipeline,
            uniform_buffer,
            bind_group,
            instance_buffer,
            instance_capacity,
        }
    }

    pub fn render(
        &mut self,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        time: f64,
        speed: f32,
        midi_file: Option<&dyn NoteSource>,
        render_state: &mut MidiRenderState,
        style: &RenderStyle,
    ) {
        let uniforms = Uniforms {
            time: time as f32,
            width: width as f32,
            height: height as f32,
            _pad: 0.0,
        };
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let mut encoder = self.device.create_command_encoder(
&wgpu::CommandEncoderDescriptor {
            label: Some("render_encoder"),
        });

        let instances = midi_file
            .map(|midi| self.build_instances(width, height, time, speed, midi, render_state, style))
            .unwrap_or_default();

        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().max(self.instance_capacity * 2);
            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instance_buffer"),
                size: (self.instance_capacity * std::mem::size_of::<NoteInstance>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if !instances.is_empty() {
                self.queue.write_buffer(
                    &self.instance_buffer,
                    0,
                    bytemuck::cast_slice(&instances),
                );
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
                pass.draw(0..6, 0..instances.len() as u32);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
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
        let pps = 200.0f64 * speed.max(0.01) as f64;
        let key_count = 128u8;
        let key_width = width as f64 / key_count as f64;

        let screen_top = height as f64 + time * pps;

        let visible_future = height as f64 / pps + 1.0;
        let visible_past = 1.0f64;
        let time_top = time + visible_future;
        let time_bottom = time - visible_past;

        if time < state.last_time {
            state.scan_indices = [0; 128];
        }
        state.last_time = time;

        let mut instances = Vec::new();

        for key in 0..128u8 {
            let notes = midi.key_notes(key);
            if notes.is_empty() {
                continue;
            }

            let mut scan = state.scan_indices[key as usize];
            while scan < notes.len() && notes[scan].end < time_bottom {
                scan += 1;
            }
            state.scan_indices[key as usize] = scan;

            let x = (key as f64 * key_width).round() as f32;
            let next_x = ((key as f64 + 1.0) * key_width).round() as f32;
            let w = (next_x - x).max(1.0);

            for i in scan..notes.len() {
                let note = &notes[i];
                if note.start > time_top {
                    break;
                }

                let y = (screen_top - note.end * pps) as f32;
                let h = ((note.end - note.start) * pps).max(1.0) as f32;

                let ch = (note.channel & 0x0F) as usize;
                let [cr, cg, cb] = style.channel_colors[ch];

                let border_px = style.border_width * w / 2.0;
                let rounding_radius = style.rounding * f32::min(w, h) / 2.0;

                // 外层：暗色描边矩形
                instances.push(NoteInstance {
                    x: x - border_px,
                    y: y - border_px,
                    w: w + 2.0 * border_px,
                    h: h + 2.0 * border_px,
                    r: cr * 0.4,
                    g: cg * 0.4,
                    b: cb * 0.4,
                    a: 0.9,
                    corner_radius: rounding_radius + border_px,
                    _pad: 0.0,
                });

                // 内层：亮色填充矩形
                instances.push(NoteInstance {
                    x,
                    y,
                    w,
                    h,
                    r: cr,
                    g: cg,
                    b: cb,
                    a: 0.9,
                    corner_radius: rounding_radius,
                    _pad: 0.0,
                });
            }
        }

        instances
    }
}
