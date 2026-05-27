use super::App;
use super::preview_layer::{self, LayerData};
use crate::piano_view;
use crate::transport::{ClipKind, LayerCommon};
use eframe::egui;
use nezha_compositor::Compositor;
use nezha_renderer::WaterfallLayer;
use std::collections::VecDeque;

/// Counter clip 的运行时统计状态。
#[derive(Clone, Debug, Default)]
pub struct CounterStats {
    /// 累计已触发的音符数。
    pub total_notes: u64,
    /// 最近每帧新增音符数（长度不超过 fps）。
    pub nps_window: VecDeque<u64>,
    /// 历史最大 NPS。
    pub max_nps: u64,
    /// 历史最大复音数。
    pub max_polyphony: u64,
    /// 上次更新的 MIDI 时间（秒）。
    pub last_midi_time: f64,
}

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

        let clip_time = (time - clip.song_start_time) as f64;
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

    /// 更新单个 Counter clip 的统计状态。
    fn update_counter_stats(
        &mut self,
        counter: &LayerData,
        midi_time: f64,
        fps: u32,
    ) -> CounterStats {
        let clip_id = counter.clip_id;
        let mut stats = self
            .counter_stats
            .remove(&clip_id)
            .unwrap_or_default();

        let Some(midi_idx) = counter.midi_idx else {
            return stats;
        };
        let Some(entry) = self.project.midi.entries.get(midi_idx) else {
            return stats;
        };
        let midi = &entry.file;

        // 向后跳跃或首次：重新计算累计数与窗口
        if midi_time < stats.last_midi_time || stats.last_midi_time == 0.0 {
            stats.total_notes = 0;
            stats.nps_window.clear();
            for key_notes in &midi.key_notes {
                for note in key_notes {
                    if note.start as f64 <= midi_time {
                        stats.total_notes += 1;
                    } else {
                        break;
                    }
                }
            }
            // 填充 NPS 窗口（近似：假设均匀分布）
            // 不精确填充，留空即可，NPS 会在后续帧自然累积
        } else if midi_time > stats.last_midi_time {
            // 向前播放：计算新增
            let mut newly_hit = 0u64;
            for key_notes in &midi.key_notes {
                for note in key_notes {
                    if note.start as f64 > stats.last_midi_time && (note.start as f64) <= midi_time {
                        newly_hit += 1;
                    } else if note.start as f64 > midi_time {
                        break;
                    }
                }
            }
            stats.total_notes += newly_hit;
            stats.nps_window.push_back(newly_hit);
            // 保持窗口不超过 fps（1 秒）
            while stats.nps_window.len() > fps as usize {
                stats.nps_window.pop_front();
            }
        }

        // 计算当前复音
        let mut polyphony = 0u64;
        for key_notes in &midi.key_notes {
            for note in key_notes {
                if note.start as f64 <= midi_time && note.end as f64 >= midi_time {
                    polyphony += 1;
                } else if note.start as f64 > midi_time {
                    break;
                }
            }
        }

        let nps: u64 = stats.nps_window.iter().sum();
        stats.max_nps = stats.max_nps.max(nps);
        stats.max_polyphony = stats.max_polyphony.max(polyphony);
        stats.last_midi_time = midi_time;

        // 存回（下面会重新取出，这里先返回副本）
        let result = stats.clone();
        self.counter_stats.insert(clip_id, stats);
        result
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
        total_instances: usize,
        fps: u32,
    ) {
        let rw = render_width as f32;
        let rh = render_height as f32;

        for counter in counter_clips {
            let midi_time = (time - counter.song_start_time).max(0.0) as f64;
            let stats = self.update_counter_stats(counter, midi_time, fps);

            let Some(midi_idx) = counter.midi_idx else {
                continue;
            };
            let Some(entry) = self.project.midi.entries.get(midi_idx) else {
                continue;
            };
            let midi = &entry.file;

            let total_sec = midi.duration;
            let rem_sec = (total_sec - midi_time).max(0.0);
            let total_frames = (total_sec * fps as f64).ceil() as u64;
            let curr_frames = (midi_time * fps as f64).round() as u64;
            let rem_frames = total_frames.saturating_sub(curr_frames);

            let curr_ticks = midi.tick_at_time(midi_time) as u64;
            let rem_ticks = midi.tick_length.saturating_sub(curr_ticks);

            let bar_divide = midi.bar_divide();
            let curr_bars = if bar_divide > 0.0 {
                (curr_ticks as f64 / bar_divide).floor() as u64 + 1
            } else {
                1
            };
            let total_bars = if bar_divide > 0.0 {
                (midi.tick_length as f64 / bar_divide).floor() as u64
            } else {
                1
            };
            let rem_bars = total_bars.saturating_sub(curr_bars);

            let bpm = midi.bpm_at_time(midi_time) as f64;
            let avg_nps = if total_sec > 0.0 {
                midi.note_count as f64 / total_sec
            } else {
                0.0
            };

            let note_percent = if midi.note_count > 0 {
                stats.total_notes as f64 / midi.note_count as f64 * 100.0
            } else {
                0.0
            };
            let tick_percent = if midi.tick_length > 0 {
                curr_ticks as f64 / midi.tick_length as f64 * 100.0
            } else {
                0.0
            };
            let time_percent = if total_sec > 0.0 {
                midi_time / total_sec * 100.0
            } else {
                0.0
            };

            let vars = nezha_text::TemplateVars {
                bpm,
                note_count: stats.total_notes,
                notes_remaining: midi.note_count.saturating_sub(stats.total_notes),
                total_notes: midi.note_count,
                nps: stats.nps_window.iter().sum(),
                max_nps: stats.max_nps,
                polyphony: stats.max_polyphony, // 当前复音：需要重新计算
                max_polyphony: stats.max_polyphony,
                total_instances: total_instances as u64,
                curr_sec: midi_time,
                curr_time: nezha_text::format_time_mmss(midi_time),
                cmil_time: nezha_text::format_time_mmss_millis(midi_time),
                cfr_time: nezha_text::format_time_mmss_frame(
                    midi_time,
                    curr_frames % fps as u64,
                    fps,
                ),
                total_sec,
                total_time: nezha_text::format_time_mmss(total_sec),
                tmil_time: nezha_text::format_time_mmss_millis(total_sec),
                tfr_time: nezha_text::format_time_mmss_frame(
                    total_sec,
                    total_frames % fps as u64,
                    fps,
                ),
                rem_sec,
                rem_time: nezha_text::format_time_mmss(rem_sec),
                rmil_time: nezha_text::format_time_mmss_millis(rem_sec),
                rfr_time: nezha_text::format_time_mmss_frame(
                    rem_sec,
                    (total_frames - curr_frames + fps as u64) % fps as u64,
                    fps,
                ),
                curr_ticks,
                total_ticks: midi.tick_length,
                rem_ticks,
                curr_bars,
                total_bars,
                rem_bars,
                ppq: midi.ticks_per_beat,
                time_sig_num: midi.time_sig_numerator,
                time_sig_den: midi.time_sig_denominator,
                avg_nps,
                curr_frames,
                total_frames,
                rem_frames,
                note_percent,
                tick_percent,
                time_percent,
            };

            // 修正：polyphony 应该是当前复音，不是 max
            // 但由于 update_counter_stats 已经更新了，我们需要再算一次当前的
            let mut current_polyphony = 0u64;
            for key_notes in &midi.key_notes {
                for note in key_notes {
                    if note.start as f64 <= midi_time && note.end as f64 >= midi_time {
                        current_polyphony += 1;
                    } else if note.start as f64 > midi_time {
                        break;
                    }
                }
            }
            let mut vars = vars;
            vars.polyphony = current_polyphony;

            let cfg = nezha_text::FormatConfig {
                separator: counter.thousand_separator,
                zero_padding: counter.zero_padding,
                ..Default::default()
            };
            let text = nezha_text::render_template(&counter.template_text, &vars, &cfg);

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
            text_layer.set_alignment(counter.text_alignment);

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
        let fps = self.project.timeline_state.fps;

        let mut compositor = Compositor::new();
        let preview_view = self.render_ctx.preview_view().clone();
        let mut is_first = true;
        let mut total_instances = 0usize;
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
                    total_instances += note_count;
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
            total_instances,
            fps,
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
