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
    pub border_width: f32,
}

/// 音符数据源抽象，解耦 renderer 与具体 MIDI 格式
pub trait NoteSource {
    /// 返回该 key 的所有音符（已按 start 排序）
    fn key_notes(&self, key: u8) -> &[nezha_core::Note];
    /// 总时长（秒）
    fn duration(&self) -> f64;
    /// PPQ (ticks per beat)，返回 None 表示无 tick 信息，降级为秒计算
    fn ticks_per_beat(&self) -> Option<u32> { None }
    /// 将秒时间转换为 tick（Tick 模式下由 tempo 决定）
    fn tick_at_time(&self, _time: f64) -> Option<f64> { None }
}

/// 渲染模式
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RenderMode {
    /// 基于秒计算音符位置（原有逻辑）
    TimeBased,
    /// 基于 MIDI tick 计算音符位置（整数 tick，无累积误差）
    /// 音符高度由 tick 长度决定，BPM 变化自动影响下落速度
    TickBased,
}

/// 渲染风格配置
#[derive(Clone)]
pub struct RenderStyle {
    /// 渲染模式
    pub render_mode: RenderMode,
    /// 边框宽度比例 0.0~1.0（1.0 表示左边 50% + 右边 50% 都是边框）
    pub border_width: f32,
    /// 圆角比例 0.0~1.0（1.0 表示底部是完全的半圆）
    pub rounding: f32,
    /// 音轨索引，用于从调色板中偏移取色
    pub track_index: usize,
    /// 16×8 调色板，128 色
    pub palette: [[f32; 3]; 128],
    /// 背景色 RGBA (0.0~1.0)，透出纯色图层
    pub background: [f64; 4],
}

impl Default for RenderStyle {
    fn default() -> Self {
        Self {
            render_mode: RenderMode::TimeBased,
            border_width: 0.1,
            rounding: 0.0,
            track_index: 0,
            palette: random_palette(),
            background: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

/// 用 golden ratio 生成 Zenith 风格的 Random 调色板
pub fn random_palette() -> [[f32; 3]; 128] {
    let mult = 0.12345f32;
    let mut palette = [[0.0f32; 3]; 128];
    for i in 0..128 {
        let hue = ((i as f32 * mult) % 1.0) * 360.0;
        let (r, g, b) = hsv_to_rgb(hue, 0.8, 1.0);
        palette[i] = [r, g, b];
    }
    palette
}


fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (r + m, g + m, b + m)
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

    fn tick_at_time(&self, time: f64) -> Option<f64> {
        Some(self.tick_at_time(time))
    }
}

/// MIDI 渲染业务状态（与 GPU 资源分离）
pub struct MidiRenderState {
    scan_indices: [usize; 128],
    last_time: f64,
    /// Tick 模式下记录上次的 scroll_tick，用于检测 seek
    last_scroll_tick: f64,
}

impl Default for MidiRenderState {
    fn default() -> Self {
        Self {
            scan_indices: [0; 128],
            last_time: -1.0,
            last_scroll_tick: -1.0,
        }
    }
}

impl MidiRenderState {
    pub fn reset(&mut self) {
        self.scan_indices = [0; 128];
        self.last_time = -1.0;
        self.last_scroll_tick = -1.0;
    }
}

/// wgpu 默认 max buffer size 约 256MB；NoteInstance = 40 bytes
const MAX_INSTANCE_COUNT: usize = 6_000_000; // ~228MB，留余量

/// GPU 资源管理 + 渲染调度
/// 支持无限音符：超出单 buffer 上限时拆分为多个 buffer，同一 render pass 内多批次 draw
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instance_buffers: Vec<wgpu::Buffer>,
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

        Self {
            device,
            queue,
            pipeline,
            uniform_buffer,
            bind_group,
            instance_buffers: Vec::new(),
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

        let instance_size = std::mem::size_of::<NoteInstance>() as u64;
        let batches: Vec<&[NoteInstance]> = instances.chunks(MAX_INSTANCE_COUNT).collect();

        // 按需创建 buffer（每个固定上限，避免超过 wgpu max_buffer_size）
        while self.instance_buffers.len() < batches.len() {
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instance_buffer"),
                size: MAX_INSTANCE_COUNT as u64 * instance_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_buffers.push(buf);
        }

        // render pass 前先把所有 batch 数据写入对应 buffer
        for (i, batch) in batches.iter().enumerate() {
            self.queue.write_buffer(
                &self.instance_buffers[i],
                0,
                bytemuck::cast_slice(batch),
            );
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: style.background[0],
                            g: style.background[1],
                            b: style.background[2],
                            a: style.background[3],
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if !instances.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                for (i, batch) in batches.iter().enumerate() {
                    pass.set_vertex_buffer(0, self.instance_buffers[i].slice(..));
                    pass.draw(0..6, 0..batch.len() as u32);
                }
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
        match style.render_mode {
            RenderMode::TimeBased => {
                self.build_instances_time(width, height, time, speed, midi, state, style)
            }
            RenderMode::TickBased => {
                self.build_instances_tick(width, height, time, speed, midi, state, style)
            }
        }
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

                let note_bottom = (screen_top - note.start * pps) as f32;
                let note_top = (screen_top - note.end * pps) as f32;
                let y = note_top;
                let h = (note_bottom - note_top).max(1.0);

                let trk = note.track as usize % 128;
                let [cr, cg, cb] = style.palette[trk];

                let border_px = style.border_width * w / 2.0;
                let rounding_radius = style.rounding * f32::min(w, h);

                instances.push(NoteInstance {
                    x,
                    y,
                    w,
                    h,
                    r: cr,
                    g: cg,
                    b: cb,
                    a: 1.0,
                    corner_radius: rounding_radius,
                    border_width: border_px,
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
        let ticks_per_beat = midi.ticks_per_beat().unwrap_or(480) as f64;
        let eff_speed = speed.max(0.01) as f64;

        // 像素/每 tick：speed 只控制视觉下落速度
        let ppt = 100.0 / ticks_per_beat * eff_speed;

        // 当前播放位置对应的 tick（由 MIDI tempo 事件自动决定）
        let scroll_tick = midi.tick_at_time(time).unwrap_or(time * ticks_per_beat * 2.0);

        // 屏幕上可见的 tick 范围
        let visible_ticks = height as f64 / ppt;
        let tick_at_bottom = scroll_tick;                          // 屏幕最底对应的 tick
        let tick_at_top = scroll_tick + visible_ticks;             // 屏幕最顶对应的 tick

        // seek 检测
        if scroll_tick < state.last_scroll_tick {
            state.scan_indices = [0; 128];
        }
        state.last_scroll_tick = scroll_tick;

        let key_count = 128u8;
        let key_width = width as f64 / key_count as f64;
        // screen_bottom = 屏幕 Y 坐标中 tick=0 的位置
        let screen_bottom = height as f64 + scroll_tick * ppt;

        let mut instances = Vec::new();

        for key in 0..128u8 {
            let notes = midi.key_notes(key);
            if notes.is_empty() {
                continue;
            }

            let mut scan = state.scan_indices[key as usize];
            // 跳过已完全滚出屏幕底部的音符 (end_tick < tick_at_bottom)
            while scan < notes.len() && (notes[scan].end_tick as f64) < tick_at_bottom {
                scan += 1;
            }
            state.scan_indices[key as usize] = scan;

            let x = (key as f64 * key_width).round() as f32;
            let next_x = ((key as f64 + 1.0) * key_width).round() as f32;
            let w = (next_x - x).max(1.0);

            for i in scan..notes.len() {
                let note = &notes[i];
                // 音符完全在屏幕上方，停止扫描
                if (note.start_tick as f64) > tick_at_top + 1.0 {
                    break;
                }

                // 上边 = end_tick（较晚的 tick = 屏幕上方），下边 = start_tick
                let note_top = (screen_bottom - note.end_tick as f64 * ppt) as f32;
                let note_bottom = (screen_bottom - note.start_tick as f64 * ppt) as f32;
                let y = note_top;
                let h = (note_bottom - note_top).max(1.0);

                let trk = note.track as usize % 128;
                let [cr, cg, cb] = style.palette[trk];

                let border_px = style.border_width * w / 2.0;
                let rounding_radius = style.rounding * f32::min(w, h);

                instances.push(NoteInstance {
                    x,
                    y,
                    w,
                    h,
                    r: cr,
                    g: cg,
                    b: cb,
                    a: 1.0,
                    corner_radius: rounding_radius,
                    border_width: border_px,
                });
            }
        }

        instances
    }
}
