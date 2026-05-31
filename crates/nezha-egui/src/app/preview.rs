use super::App;
use super::preview_layer::{self, LayerData};
use crate::piano_view;
use crate::transport::{ClipKind, LayerCommon};
use eframe::egui;
use nezha_compositor::{BlendMode, Compositor};
use nezha_renderer::WaterfallLayer;

#[derive(Clone, Debug, Default)]
pub struct CounterStats {
    pub total_notes: u64,
    pub max_nps: u64,
    pub max_polyphony: u64,
    pub last_midi_time: f64,
}

struct LayerRenderContext<'a> {
    compositor: &'a mut Compositor,
    preview_view: &'a wgpu::TextureView,
    time: f32,
    render_width: u32,
    render_height: u32,
}

impl App {
    fn default_style(&self) -> nezha_renderer::RenderStyle {
        nezha_renderer::RenderStyle {
            palette: nezha_renderer::random_palette(),
            ..Default::default()
        }
    }

    fn render_frame_common(&mut self, time: f32, submit_export: bool) {
        let render_width = self.project.render.width;
        let render_height = self.project.render.height;
        self.render_ctx
            .ensure_preview_size(render_width, render_height);

        if submit_export {
            self.export_pipeline
                .ensure_size(render_width, render_height);
        }

        self.render_ctx.begin_pass();
        self.render_all_layers(time, render_width, render_height);

        if submit_export {
            let encoder = self.render_ctx.take_encoder();
            let texture = self.render_ctx.preview_texture();
            let queue = self.render_ctx.queue();
            self.export_pipeline
                .copy_and_submit(encoder, texture, queue);
        }
    }

    pub(super) fn render_frame_for_export(&mut self, time: f32) {
        self.render_frame_common(time, false);
        self.render_ctx.end_pass();
    }

    pub(super) fn render_frame_combined(&mut self, time: f32) -> Vec<u8> {
        self.render_frame_common(time, true);
        self.export_pipeline.wait_read()
    }

    pub(super) fn render_frame_pipelined(&mut self, time: f32) {
        self.render_frame_common(time, true);
    }

    fn render_solid_color_layer(
        &mut self,
        ctx: &mut LayerRenderContext<'_>,
        clip: &LayerData,
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
        let blend_mode = clip.common.blend_mode;
        let preview_view = ctx.preview_view;
        let render_width = ctx.render_width;
        let render_height = ctx.render_height;
        let time = ctx.time;
        self.render_ctx
            .with_solid_color_layer(color, |solid, encoder| {
                ctx.compositor.render_layer(
                    encoder,
                    solid,
                    preview_view,
                    render_width,
                    render_height,
                    time as f64,
                    load_op,
                    blend_mode,
                    rect,
                );
            });
    }

    fn render_waterfall_layer(
        &mut self,
        ctx: &mut LayerRenderContext<'_>,
        clip: &LayerData,
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

        let clip_time = (ctx.time - clip.song_start_time) as f64;
        let keyboard_height_px = ctx.render_height as f32 * clip.keyboard_height_percent;
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
            ctx.render_width,
            clip.equal_key_width,
        );
        renderer.prepare(
            ctx.render_width,
            ctx.render_height,
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
                ctx.compositor.render_layer(
                    encoder,
                    &mut wrapper,
                    ctx.preview_view,
                    ctx.render_width,
                    ctx.render_height,
                    clip_time,
                    load_op,
                    blend_mode,
                    rect,
                );
            });
        note_count
    }

    fn render_image_layer(
        &mut self,
        ctx: &mut LayerRenderContext<'_>,
        clip: &LayerData,
        rect: (f32, f32, f32, f32),
        is_first: bool,
    ) {
        let Some(media_idx) = clip.media_idx else {
            return;
        };
        let Some(entry) = self.project.media.get(media_idx) else {
            return;
        };
        let (Some(rgba), w, h) = (&entry.image_rgba, entry.image_width, entry.image_height) else {
            return;
        };

        let load_op = if is_first {
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
        } else {
            wgpu::LoadOp::Load
        };

        if let Some(layer) = self.image_layer_cache.get_mut(&media_idx) {
            layer.update_texture(
                self.render_ctx.device(),
                self.render_ctx.queue(),
                rgba,
                w,
                h,
            );
            let encoder = self.render_ctx.encoder_mut();
            ctx.compositor.render_layer(
                encoder,
                layer,
                ctx.preview_view,
                ctx.render_width,
                ctx.render_height,
                ctx.time as f64,
                load_op,
                clip.common.blend_mode,
                rect,
            );
        } else {
            let mut layer = nezha_compositor::ImageLayer::new(
                self.render_ctx.device(),
                self.render_ctx.queue(),
                self.render_ctx.target_format(),
                rgba,
                w,
                h,
            );
            let encoder = self.render_ctx.encoder_mut();
            ctx.compositor.render_layer(
                encoder,
                &mut layer,
                ctx.preview_view,
                ctx.render_width,
                ctx.render_height,
                ctx.time as f64,
                load_op,
                clip.common.blend_mode,
                rect,
            );
            self.image_layer_cache.insert(media_idx, layer);
        }
    }

    fn render_video_layer(
        &mut self,
        ctx: &mut LayerRenderContext<'_>,
        clip: &LayerData,
        rect: (f32, f32, f32, f32),
        is_first: bool,
    ) {
        let Some(media_idx) = clip.media_idx else {
            return;
        };
        let Some(entry) = self.project.media.get(media_idx) else {
            return;
        };
        let w = entry.info.width;
        let h = entry.info.height;
        if w == 0 || h == 0 {
            return;
        }

        let clip_time = (ctx.time - clip.start).max(0.0) as f64;
        let frame = match self.project.media.get_video_frame(media_idx, clip_time) {
            Some(f) => f,
            None => return,
        };

        let load_op = if is_first {
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
        } else {
            wgpu::LoadOp::Load
        };

        if let Some(layer) = self.video_layer_cache.get_mut(&media_idx) {
            layer.update_texture(
                self.render_ctx.device(),
                self.render_ctx.queue(),
                &frame.rgba,
                frame.width,
                frame.height,
            );
            let encoder = self.render_ctx.encoder_mut();
            ctx.compositor.render_layer(
                encoder,
                layer,
                ctx.preview_view,
                ctx.render_width,
                ctx.render_height,
                ctx.time as f64,
                load_op,
                clip.common.blend_mode,
                rect,
            );
        } else {
            let mut layer = nezha_compositor::ImageLayer::new(
                self.render_ctx.device(),
                self.render_ctx.queue(),
                self.render_ctx.target_format(),
                &frame.rgba,
                frame.width,
                frame.height,
            );
            let encoder = self.render_ctx.encoder_mut();
            ctx.compositor.render_layer(
                encoder,
                &mut layer,
                ctx.preview_view,
                ctx.render_width,
                ctx.render_height,
                ctx.time as f64,
                load_op,
                clip.common.blend_mode,
                rect,
            );
            self.video_layer_cache.insert(media_idx, layer);
        }
    }

    /// Compute MIDI statistics using binary search per key.
    ///
    /// For black MIDI with millions of notes, the old O(total_notes) scan was
    /// catastrophic. This uses `partition_point` for `total_notes` and `nps`,
    /// and a bounded scan for `polyphony`.
    fn compute_midi_stats(midi: &nezha_core::MidiFile, midi_time: f64) -> (u64, u64, u64) {
        let mut total_notes = 0u64;
        let mut polyphony = 0u64;
        let mut nps = 0u64;
        let window_start = (midi_time - 1.0).max(0.0);

        for key_notes in &midi.key_notes {
            if key_notes.is_empty() {
                continue;
            }

            // Binary search: first note with start > midi_time
            let upper = key_notes.partition_point(|n| n.start > midi_time);
            total_notes += upper as u64;

            // Binary search: first note with start > window_start
            let lower = key_notes.partition_point(|n| n.start > window_start);
            nps += (upper - lower) as u64;

            // Polyphony: scan notes in [lower..upper] for end >= midi_time.
            // Since notes overlap randomly, we must check each one in this range.
            // For black MIDI, this range is typically small relative to total notes.
            for note in &key_notes[lower..upper] {
                if note.end >= midi_time {
                    polyphony += 1;
                }
            }
        }

        (total_notes, polyphony, nps)
    }

    fn update_counter_stats(&mut self, counter: &LayerData, midi_time: f64) -> CounterStats {
        let clip_id = counter.clip_id;
        let mut stats = self.counter_stats.remove(&clip_id).unwrap_or_default();

        let Some(midi_idx) = counter.midi_idx else {
            return stats;
        };
        let Some(entry) = self.project.midi.entries.get(midi_idx) else {
            return stats;
        };
        let midi = &entry.file;

        let (total_notes, polyphony, nps) = Self::compute_midi_stats(midi, midi_time);
        stats.total_notes = total_notes;
        stats.max_nps = stats.max_nps.max(nps);
        stats.max_polyphony = stats.max_polyphony.max(polyphony);
        stats.last_midi_time = midi_time;

        let result = stats.clone();
        self.counter_stats.insert(clip_id, stats);
        result
    }

    fn render_counter_layers(
        &mut self,
        ctx: &mut LayerRenderContext<'_>,
        counter_clips: &[LayerData],
        make_rect: &impl Fn(&LayerCommon, f32, f32) -> (f32, f32, f32, f32),
        total_instances: usize,
        fps: u32,
    ) {
        let rw = ctx.render_width as f32;
        let rh = ctx.render_height as f32;

        for counter in counter_clips {
            let midi_time = (ctx.time - counter.song_start_time).max(0.0) as f64;
            let stats = self.update_counter_stats(counter, midi_time);

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

            let (_, current_polyphony, current_nps) = Self::compute_midi_stats(midi, midi_time);

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
                nps: current_nps,
                max_nps: stats.max_nps,
                polyphony: current_polyphony,
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
            let outline_color_f = [
                counter.outline_color.r() as f32 / 255.0,
                counter.outline_color.g() as f32 / 255.0,
                counter.outline_color.b() as f32 / 255.0,
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
            if counter.outline_enabled {
                text_layer.set_outline(counter.outline_width, outline_color_f);
            }
            text_layer.set_bold(counter.bold);
            text_layer.set_bold_offset(counter.bold_offset);
            text_layer.set_italic(counter.italic);
            text_layer.set_italic_slant(counter.italic_slant);
            text_layer.set_letter_spacing(counter.letter_spacing);
            text_layer.set_min_advance(counter.min_advance);

            let rect = make_rect(&counter.common, rw, rh);
            let encoder = self.render_ctx.encoder_mut();
            ctx.compositor.render_layer(
                encoder,
                &mut text_layer,
                ctx.preview_view,
                ctx.render_width,
                ctx.render_height,
                ctx.time as f64,
                wgpu::LoadOp::Load,
                counter.common.blend_mode,
                rect,
            );
        }
    }

    fn render_all_layers(&mut self, time: f32, render_width: u32, render_height: u32) {
        let layers =
            preview_layer::collect_visible_layers(&self.project.timeline_state.data.tracks, time);
        let default_style = self.default_style();
        let fps = self.project.timeline_state.fps;

        let mut compositor = Compositor::new();
        let preview_view = self.render_ctx.preview_view().clone();
        let mut ctx = LayerRenderContext {
            compositor: &mut compositor,
            preview_view: &preview_view,
            time,
            render_width,
            render_height,
        };
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
                    self.render_solid_color_layer(&mut ctx, clip, rect, is_first);
                    is_first = false;
                }
                ClipKind::Waterfall => {
                    let note_count = self.render_waterfall_layer(
                        &mut ctx,
                        clip,
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
                ClipKind::Audio => {}
                ClipKind::Image => {
                    self.render_image_layer(&mut ctx, clip, rect, is_first);
                    is_first = false;
                }
                ClipKind::Video => {
                    self.render_video_layer(&mut ctx, clip, rect, is_first);
                    is_first = false;
                }
            }
        }

        // 如果没有任何图层被渲染，清除为黑色
        if is_first {
            self.render_ctx.with_solid_color_layer([0.0, 0.0, 0.0, 1.0], |solid, encoder| {
                ctx.compositor.render_layer(
                    encoder,
                    solid,
                    ctx.preview_view,
                    ctx.render_width,
                    ctx.render_height,
                    ctx.time as f64,
                    wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    BlendMode::Normal,
                    (0.0, 0.0, 1.0, 1.0),
                );
            });
        }

        self.render_counter_layers(&mut ctx, &counter_clips, &make_rect, total_instances, fps);
    }

    pub(super) fn render_preview(&mut self, ui: &mut egui::Ui) {
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
