use super::App;
use crate::piano_view;
use crate::transport::ClipKind;
use eframe::egui;
use nezha_compositor::{Compositor, LayerRenderer};

/// Wrapper to adapt [`nezha_renderer::Renderer`] for the compositor's [`LayerRenderer`] trait.
struct WaterfallLayer<'a> {
    renderer: &'a nezha_renderer::Renderer,
}

impl LayerRenderer for WaterfallLayer<'_> {
    fn prepare(&mut self, _width: u32, _height: u32, _time: f64) {
        // Preparation is done externally before wrapping.
    }

    fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        _time: f64,
        load_op: wgpu::LoadOp<wgpu::Color>,
        _blend_mode: nezha_compositor::BlendMode,
        rect: (f32, f32, f32, f32),
    ) {
        self.renderer
            .draw(encoder, target, width, height, load_op, rect);
    }
}

/// 图层渲染所需数据（复制自 TrackClip，避免持有 self 的引用）。
#[derive(Clone)]
struct LayerData {
    clip_id: usize,
    kind: ClipKind,
    midi_idx: Option<usize>,
    speed: f32,
    border_width: f32,
    rounding: f32,
    render_mode: nezha_renderer::RenderMode,
    equal_key_width: bool,
    clip_start: f32,
    color: egui::Color32,
    keyboard_height_percent: f32,
}

impl App {
    /// 收集当前时间点所有可见图层数据（Premiere 顺序：底 -> 顶）。
    fn collect_visible_layers(&self, time: f32) -> Vec<LayerData> {
        let mut layers = Vec::new();
        for track in self.project.timeline_state.data.tracks.iter().rev() {
            for clip in &track.clips {
                if time >= clip.start && time < clip.end {
                    layers.push(LayerData {
                        clip_id: clip.id,
                        kind: clip.kind,
                        midi_idx: clip.midi_idx,
                        speed: clip.speed,
                        border_width: clip.border_width,
                        rounding: clip.rounding,
                        render_mode: clip.render_mode,
                        equal_key_width: clip.equal_key_width,
                        clip_start: clip.start,
                        color: clip.color,
                        keyboard_height_percent: clip.keyboard_height_percent,
                    });
                }
            }
        }
        layers
    }

    fn default_style(&self) -> nezha_renderer::RenderStyle {
        nezha_renderer::RenderStyle {
            palette: nezha_renderer::random_palette(),
            ..Default::default()
        }
    }

    /// 渲染指定时间点的画面到预览目标（不显示到 UI）。
    ///
    /// 用于预览路径，每层单独 begin_pass/end_pass。
    pub(super) fn render_frame_for_export(&mut self, time: f32) {
        let render_width = self.project.render.width;
        let render_height = self.project.render.height;
        self.render_ctx
            .ensure_preview_size(render_width, render_height);
        self.render_ctx.begin_pass();
        self.render_all_layers(time, render_width, render_height);
        self.render_ctx.end_pass();
    }

    /// 渲染一帧并立即同步读回像素数据。
    ///
    /// 将所有图层渲染 + 纹理拷贝合并到单个 CommandEncoder，
    /// 使用 triple buffering ring 的单槽快速路径。
    /// 返回 BGRA 像素数据。
    #[allow(dead_code)]
    pub(super) fn render_frame_combined(&mut self, time: f32) -> Vec<u8> {
        let render_width = self.project.render.width;
        let render_height = self.project.render.height;
        self.render_ctx
            .ensure_preview_size(render_width, render_height);

        // 单次 begin_pass：所有图层 + copy 共用同一 encoder
        self.render_ctx.begin_pass();
        self.render_all_layers(time, render_width, render_height);

        // 使用 ring API：copy → submit+map → wait_read
        let slot = self
            .render_ctx
            .copy_frame_to_staging_ring(render_width, render_height);
        self.render_ctx.submit_and_map_staging(slot);
        self.render_ctx.wait_read_staging()
    }

    /// 将当前帧渲染并推入 staging ring（不阻塞等待读回）。
    ///
    /// 用于流水线导出：调用后可通过 try_read_staging / wait_read_staging 获取数据。
    pub(super) fn render_frame_pipelined(&mut self, time: f32) {
        let render_width = self.project.render.width;
        let render_height = self.project.render.height;
        self.render_ctx
            .ensure_preview_size(render_width, render_height);

        self.render_ctx.begin_pass();
        self.render_all_layers(time, render_width, render_height);

        let slot = self
            .render_ctx
            .copy_frame_to_staging_ring(render_width, render_height);
        self.render_ctx.submit_and_map_staging(slot);
    }

    /// 渲染所有可见图层到当前 frame encoder（不创建/提交 encoder）。
    fn render_all_layers(&mut self, time: f32, render_width: u32, render_height: u32) {
        let layers = self.collect_visible_layers(time);
        let default_style = self.default_style();

        let mut compositor = Compositor::new();
        let preview_view = self.render_ctx.preview_view().clone();
        let mut is_first = true;

        for clip in &layers {
            match clip.kind {
                ClipKind::SolidColor => {
                    let color = [
                        clip.color.r() as f64 / 255.0,
                        clip.color.g() as f64 / 255.0,
                        clip.color.b() as f64 / 255.0,
                        1.0,
                    ];
                    let load_op = if is_first {
                        wgpu::LoadOp::Clear(wgpu::Color {
                            r: color[0],
                            g: color[1],
                            b: color[2],
                            a: color[3],
                        })
                    } else {
                        wgpu::LoadOp::Load
                    };
                    let mut solid = nezha_compositor::SolidColorLayer::new(
                        self.render_ctx.device(),
                        self.render_ctx.queue(),
                        self.render_ctx.target_format(),
                        color,
                    );
                    let encoder = self.render_ctx.encoder_mut();
                    compositor.render_layer(
                        encoder,
                        &mut solid,
                        &preview_view,
                        render_width,
                        render_height,
                        time as f64,
                        load_op,
                        nezha_compositor::BlendMode::Normal,
                        (0.0, 0.0, 1.0, 1.0),
                    );
                    is_first = false;
                }
                ClipKind::Waterfall => {
                    let Some(midi_idx) = clip.midi_idx else {
                        continue;
                    };
                    let Some(entry) = self.project.midi.entries.get(midi_idx) else {
                        continue;
                    };

                    let clip_time = (time - clip.clip_start).max(0.0) as f64;
                    let keyboard_height_px = render_height as f32 * clip.keyboard_height_percent;
                    let clip_style = nezha_renderer::RenderStyle {
                        render_mode: clip.render_mode,
                        border_width: clip.border_width,
                        rounding: clip.rounding,
                        track_index: 0,
                        palette: default_style.palette,
                        background: [0.0, 0.0, 0.0, 0.0],
                        equal_key_width: clip.equal_key_width,
                        keyboard_height: keyboard_height_px,
                    };

                    let load_op = if is_first {
                        wgpu::LoadOp::Clear(wgpu::Color {
                            r: clip_style.background[0],
                            g: clip_style.background[1],
                            b: clip_style.background[2],
                            a: clip_style.background[3],
                        })
                    } else {
                        wgpu::LoadOp::Load
                    };

                    self.render_ctx
                        .get_or_create_renderer(
                            clip.clip_id,
                            midi_idx,
                            &entry.file,
                            render_width,
                            clip.equal_key_width,
                        )
                        .prepare(
                            render_width,
                            render_height,
                            clip_time,
                            clip.speed,
                            Some(&entry.file),
                            &clip_style,
                        );

                    self.render_ctx
                        .with_waterfall_renderer(clip.clip_id, |renderer, encoder| {
                            let mut wrapper = WaterfallLayer { renderer };
                            compositor.render_layer(
                                encoder,
                                &mut wrapper,
                                &preview_view,
                                render_width,
                                render_height,
                                clip_time,
                                load_op,
                                nezha_compositor::BlendMode::Normal,
                                (0.0, 0.0, 1.0, 1.0),
                            );
                        });
                    is_first = false;
                }
            }
        }
    }

    pub(super) fn render_preview(&mut self, ui: &mut egui::Ui) {
        // 导出期间由 export_step 控制画面渲染，此处仅做显示
        if self.export_state.is_none() {
            self.update_playback();
            let current_time = self.project.playback.current_time as f32;
            self.render_frame_for_export(current_time);
        }

        let available = ui.available_size();
        let render_width = self.project.render.width;
        let render_height = self.project.render.height;
        let aspect = render_width as f32 / render_height as f32;

        self.ui.zoom = piano_view::show(
            ui,
            self.render_ctx.preview_texture_id(),
            available,
            aspect,
            &mut self.ui.zoom,
            &mut self.ui.pan_offset,
        );
    }
}
