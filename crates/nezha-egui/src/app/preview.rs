use super::App;
use super::preview_layer::{self, LayerData};
use crate::piano_view;
use crate::transport::{ClipKind, LayerCommon};
use eframe::egui;
use nezha_compositor::Compositor;
use nezha_renderer::WaterfallLayer;

impl App {
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
        self.export_pipeline
            .ensure_size(render_width, render_height);

        self.render_ctx.begin_pass();
        self.render_all_layers(time, render_width, render_height);

        let encoder = self.render_ctx.take_encoder();
        let texture = self.render_ctx.preview_texture();
        let queue = self.render_ctx.queue();
        self.export_pipeline
            .copy_and_submit(encoder, texture, queue);
        self.export_pipeline.wait_read()
    }

    /// 将当前帧渲染并推入 staging ring（不阻塞等待读回）。
    ///
    /// 用于流水线导出：调用后可通过 try_read_staging / wait_read_staging 获取数据。
    pub(super) fn render_frame_pipelined(&mut self, time: f32) {
        let render_width = self.project.render.width;
        let render_height = self.project.render.height;
        self.render_ctx
            .ensure_preview_size(render_width, render_height);
        self.export_pipeline
            .ensure_size(render_width, render_height);

        self.render_ctx.begin_pass();
        self.render_all_layers(time, render_width, render_height);

        let encoder = self.render_ctx.take_encoder();
        let texture = self.render_ctx.preview_texture();
        let queue = self.render_ctx.queue();
        self.export_pipeline
            .copy_and_submit(encoder, texture, queue);
    }

    /// Render a solid color clip into the compositor.
    #[allow(clippy::too_many_arguments)]
    fn render_solid_color_layer(
        &mut self,
        compositor: &mut Compositor,
        preview_view: &wgpu::TextureView,
        clip: &LayerData,
        time: f32,
        render_width: u32,
        render_height: u32,
        rect: (f32, f32, f32, f32),
        is_first: bool,
    ) {
        let opacity = clip.common.opacity as f64;
        let color = [
            clip.color.r() as f64 / 255.0,
            clip.color.g() as f64 / 255.0,
            clip.color.b() as f64 / 255.0,
            opacity,
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
            preview_view,
            render_width,
            render_height,
            time as f64,
            load_op,
            clip.common.blend_mode,
            rect,
        );
    }

    /// Render a waterfall (note visualization) clip into the compositor.
    #[allow(clippy::too_many_arguments)]
    fn render_waterfall_layer(
        &mut self,
        compositor: &mut Compositor,
        preview_view: &wgpu::TextureView,
        clip: &LayerData,
        time: f32,
        render_width: u32,
        render_height: u32,
        rect: (f32, f32, f32, f32),
        default_palette: [[f32; 3]; 128],
        is_first: bool,
    ) -> usize {
        let Some(midi_idx) = clip.midi_idx else {
            return 0;
        };
        let Some(entry) = self.project.midi.entries.get(midi_idx) else {
            return 0;
        };

        let clip_time = (time - clip.clip_start).max(0.0) as f64;
        let keyboard_height_px = render_height as f32 * clip.keyboard_height_percent;
        let opacity = clip.common.opacity as f64;
        let clip_style = nezha_renderer::RenderStyle {
            render_mode: clip.render_mode,
            border_width: clip.border_width,
            rounding: clip.rounding,
            track_index: 0,
            palette: default_palette,
            background: [0.0, 0.0, 0.0, opacity],
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

        let renderer = self.render_ctx.get_or_create_renderer(
            clip.clip_id,
            midi_idx,
            &entry.file,
            render_width,
            clip.equal_key_width,
        );
        renderer.prepare(
            render_width,
            render_height,
            clip_time,
            clip.speed,
            Some(&entry.file),
            &clip_style,
        );
        let note_count = renderer.total_instances();

        let blend_mode = clip.common.blend_mode;
        self.render_ctx
            .with_waterfall_renderer(clip.clip_id, |renderer, encoder| {
                let mut wrapper = WaterfallLayer { renderer };
                compositor.render_layer(
                    encoder,
                    &mut wrapper,
                    preview_view,
                    render_width,
                    render_height,
                    clip_time,
                    load_op,
                    blend_mode,
                    rect,
                );
            });
        note_count
    }

    /// Render all counter (text overlay) clips after other layers.
    #[allow(clippy::too_many_arguments)]
    fn render_counter_layers(
        &mut self,
        compositor: &mut Compositor,
        preview_view: &wgpu::TextureView,
        counter_clips: &[LayerData],
        time: f32,
        render_width: u32,
        render_height: u32,
        make_rect: &impl Fn(&LayerCommon, f32, f32) -> (f32, f32, f32, f32),
        total_notes: usize,
    ) {
        let rw = render_width as f32;
        let rh = render_height as f32;
        for counter in counter_clips {
            let text = format!("{:.2}s | Notes: {}", time, total_notes);
            let opacity = counter.common.opacity;
            let color_f = [
                counter.text_color.r() as f32 / 255.0,
                counter.text_color.g() as f32 / 255.0,
                counter.text_color.b() as f32 / 255.0,
                opacity,
            ];

            let mut text_layer = nezha_text::TextLayer::new(
                &mut self.font_atlas,
                self.render_ctx.device(),
                self.render_ctx.queue(),
                self.render_ctx.target_format(),
            );
            text_layer.set_text(text);
            text_layer.set_position([counter.common.position_x, counter.common.position_y]);
            text_layer.set_font_size(counter.font_size);
            text_layer.set_color(color_f);

            let rect = make_rect(&counter.common, rw, rh);
            let encoder = self.render_ctx.encoder_mut();
            compositor.render_layer(
                encoder,
                &mut text_layer,
                preview_view,
                render_width,
                render_height,
                time as f64,
                wgpu::LoadOp::Load,
                counter.common.blend_mode,
                rect,
            );
        }
    }

    /// Render all visible layers into the current frame encoder.
    fn render_all_layers(&mut self, time: f32, render_width: u32, render_height: u32) {
        let layers =
            preview_layer::collect_visible_layers(&self.project.timeline_state.data.tracks, time);
        let default_style = self.default_style();

        let mut compositor = Compositor::new();
        let preview_view = self.render_ctx.preview_view().clone();
        let mut is_first = true;
        let mut total_notes = 0usize;
        let mut counter_clips: Vec<LayerData> = Vec::new();

        let make_rect = |c: &LayerCommon, w: f32, h: f32| -> (f32, f32, f32, f32) {
            (
                c.position_x / w,
                c.position_y / h,
                c.scale_x.abs(),
                c.scale_y.abs(),
            )
        };
        let rw = render_width as f32;
        let rh = render_height as f32;

        for clip in &layers {
            if clip.kind == ClipKind::Counter {
                counter_clips.push(clip.clone());
                continue;
            }

            let rect = make_rect(&clip.common, rw, rh);

            match clip.kind {
                ClipKind::SolidColor => {
                    self.render_solid_color_layer(
                        &mut compositor,
                        &preview_view,
                        clip,
                        time,
                        render_width,
                        render_height,
                        rect,
                        is_first,
                    );
                    is_first = false;
                }
                ClipKind::Waterfall => {
                    let note_count = self.render_waterfall_layer(
                        &mut compositor,
                        &preview_view,
                        clip,
                        time,
                        render_width,
                        render_height,
                        rect,
                        default_style.palette,
                        is_first,
                    );
                    total_notes += note_count;
                    is_first = false;
                }
                ClipKind::Counter => {
                    unreachable!();
                }
                ClipKind::Audio => {
                    // Audio clips are not rendered visually; only played back
                }
            }
        }

        self.render_counter_layers(
            &mut compositor,
            &preview_view,
            &counter_clips,
            time,
            render_width,
            render_height,
            &make_rect,
            total_notes,
        );
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
