use super::preview_layer::{self, LayerData};
use super::project_state::ProjectState;
use super::render_context::export::ExportPipeline;
use super::render_context::RenderContext;
use crate::transport::{ClipKind, LayerCommon};
use nezha_compositor::BlendMode;
use nezha_renderer::WaterfallLayer;
use nezha_text::prepare_text;

#[derive(Clone, Debug, Default)]
pub struct CounterStats {
    pub total_notes: u64,
    pub max_nps: u64,
    pub max_polyphony: u64,
    pub last_midi_time: f64,
}

struct LayerRenderContext<'a> {
    preview_view: &'a wgpu::TextureView,
    time: f32,
    render_width: u32,
    render_height: u32,
}

pub(crate) struct PreviewRenderer {
    pub render_ctx: RenderContext,
    pub export_pipeline: ExportPipeline,
    pub font_atlas: nezha_text::FontAtlas,
    pub counter_stats: std::collections::HashMap<usize, CounterStats>,
    pub video_layer_cache: std::collections::HashMap<usize, nezha_compositor::ImageLayer>,
    pub image_layer_cache: std::collections::HashMap<usize, nezha_compositor::ImageLayer>,
    pub text_layer_cache: std::collections::HashMap<usize, nezha_text::TextLayer>,
}

impl PreviewRenderer {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let wgpu_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("wgpu backend required");
        let font =
            nezha_text::FontRef::from_bytes(include_bytes!("../../../../assets/MiSans-Regular.otf"))
                .expect("failed to load MiSans font");
        let font_atlas = nezha_text::FontAtlas::new(&wgpu_state.device, &wgpu_state.queue, font);

        let default_w = super::constants::DEFAULT_PREVIEW_WIDTH;
        let default_h = super::constants::DEFAULT_PREVIEW_HEIGHT;

        Self {
            render_ctx: RenderContext::new(cc, default_w, default_h),
            export_pipeline: ExportPipeline::new(&wgpu_state.device, default_w, default_h),
            font_atlas,
            counter_stats: std::collections::HashMap::new(),
            video_layer_cache: std::collections::HashMap::new(),
            image_layer_cache: std::collections::HashMap::new(),
            text_layer_cache: std::collections::HashMap::new(),
        }
    }

    pub fn reset_midi_state(&mut self) {
        self.render_ctx.reset_midi_state();
    }

    // ── Frame rendering entry points ──

    fn render_frame_common(
        &mut self,
        time: f32,
        submit_export: bool,
        project: &mut ProjectState,
    ) {
        let render_width = project.render.width;
        let render_height = project.render.height;
        self.render_ctx
            .ensure_preview_size(render_width, render_height);

        if submit_export {
            self.export_pipeline
                .ensure_size(render_width, render_height);
        }

        self.render_ctx.begin_pass();
        self.render_all_layers(time, render_width, render_height, project);

        if submit_export {
            let encoder = self.render_ctx.take_encoder();
            let texture = self.render_ctx.preview_texture();
            let queue = self.render_ctx.queue();
            self.export_pipeline
                .copy_and_submit(encoder, texture, queue);
        }
    }

    pub fn render_frame_for_export(&mut self, time: f32, project: &mut ProjectState) {
        self.render_frame_common(time, false, project);
        self.render_ctx.end_pass();
    }

    pub fn render_frame_combined(&mut self, time: f32, project: &mut ProjectState) -> Vec<u8> {
        self.render_frame_common(time, true, project);
        self.export_pipeline.wait_read()
    }

    pub fn render_frame_pipelined(&mut self, time: f32, project: &mut ProjectState) {
        self.render_frame_common(time, true, project);
    }

    // ── Layer rendering ──

    fn clear_or_load(is_first: bool) -> wgpu::LoadOp<wgpu::Color> {
        if is_first {
            wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 })
        } else {
            wgpu::LoadOp::Load
        }
    }

    fn default_style() -> nezha_renderer::RenderStyle {
        nezha_renderer::RenderStyle {
            palette: nezha_renderer::random_palette(),
            ..Default::default()
        }
    }

    fn render_solid_color_layer(
        render_ctx: &mut RenderContext,
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
        render_ctx.with_solid_color_layer(color, |solid, encoder| {
            nezha_compositor::render_layer(
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
        render_ctx: &mut RenderContext,
        project: &mut ProjectState,
        ctx: &mut LayerRenderContext<'_>,
        clip: &LayerData,
        rect: (f32, f32, f32, f32),
        default_palette: [[f32; 3]; 128],
        is_first: bool,
    ) -> usize {
        let Some(midi_idx) = clip.midi_idx else {
            return 0;
        };
        let Some(entry) = project.midi.entries.get(midi_idx) else {
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

        let renderer = render_ctx.get_or_create_renderer(
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
        render_ctx.with_waterfall_renderer(clip.clip_id, |renderer, encoder| {
            let mut wrapper = WaterfallLayer { renderer };
            nezha_compositor::render_layer(
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

    fn render_media_layer(
        render_ctx: &mut RenderContext,
        cache: &mut std::collections::HashMap<usize, nezha_compositor::ImageLayer>,
        media_idx: usize,
        rgba: &[u8],
        w: u32,
        h: u32,
        ctx: &mut LayerRenderContext<'_>,
        blend_mode: BlendMode,
        rect: (f32, f32, f32, f32),
        load_op: wgpu::LoadOp<wgpu::Color>,
    ) {
        if let Some(layer) = cache.get_mut(&media_idx) {
            layer.update_texture(render_ctx.device(), render_ctx.queue(), rgba, w, h);
            let encoder = render_ctx.encoder_mut();
            nezha_compositor::render_layer(
                encoder,
                layer,
                ctx.preview_view,
                ctx.render_width,
                ctx.render_height,
                ctx.time as f64,
                load_op,
                blend_mode,
                rect,
            );
        } else {
            let mut layer = nezha_compositor::ImageLayer::new(
                render_ctx.device(),
                render_ctx.queue(),
                render_ctx.target_format(),
                rgba,
                w,
                h,
            );
            let encoder = render_ctx.encoder_mut();
            nezha_compositor::render_layer(
                encoder,
                &mut layer,
                ctx.preview_view,
                ctx.render_width,
                ctx.render_height,
                ctx.time as f64,
                load_op,
                blend_mode,
                rect,
            );
            cache.insert(media_idx, layer);
        }
    }

    fn render_media_clip(
        render_ctx: &mut RenderContext,
        cache: &mut std::collections::HashMap<usize, nezha_compositor::ImageLayer>,
        project: &mut ProjectState,
        ctx: &mut LayerRenderContext<'_>,
        clip: &LayerData,
        rect: (f32, f32, f32, f32),
        is_first: bool,
    ) {
        let Some(media_idx) = clip.media_idx else {
            return;
        };

        // Static image: has pre-decoded RGBA
        if let Some(entry) = project.media.get(media_idx)
            && let Some(ref rgba) = entry.image_rgba
        {
            Self::render_media_layer(
                render_ctx,
                cache,
                media_idx,
                rgba,
                entry.image_width,
                entry.image_height,
                ctx,
                clip.common.blend_mode,
                rect,
                Self::clear_or_load(is_first),
            );
            return;
        }

        // Video: decode frame at current time (get_video_frame needs &mut)
        let clip_time = (ctx.time - clip.start).max(0.0) as f64;
        if let Some(frame) = project.media.get_video_frame(media_idx, clip_time) {
            Self::render_media_layer(
                render_ctx,
                cache,
                media_idx,
                &frame.rgba,
                frame.width,
                frame.height,
                ctx,
                clip.common.blend_mode,
                rect,
                Self::clear_or_load(is_first),
            );
        }
    }

    // ── Counter stats ──

    fn compute_midi_stats(midi: &nezha_core::MidiFile, midi_time: f64) -> (u64, u64, u64) {
        let mut total_notes = 0u64;
        let mut polyphony = 0u64;
        let mut nps = 0u64;
        let window_start = (midi_time - 1.0).max(0.0);

        for key_notes in &midi.key_notes {
            if key_notes.is_empty() {
                continue;
            }

            let upper = key_notes.partition_point(|n| n.start <= midi_time);
            total_notes += upper as u64;

            let lower = key_notes.partition_point(|n| n.start <= window_start);
            nps += (upper - lower) as u64;

            for note in &key_notes[lower..upper] {
                if note.end >= midi_time {
                    polyphony += 1;
                }
            }
        }

        (total_notes, polyphony, nps)
    }

    fn update_counter_stats(
        counter_stats: &mut std::collections::HashMap<usize, CounterStats>,
        project: &mut ProjectState,
        counter: &LayerData,
        midi_time: f64,
    ) -> (CounterStats, u64, u64) {
        let clip_id = counter.clip_id;

        if counter.midi_idx.is_none() {
            return (CounterStats::default(), 0, 0);
        }
        let midi_idx = counter.midi_idx.unwrap();
        let Some(entry) = project.midi.entries.get(midi_idx) else {
            return (CounterStats::default(), 0, 0);
        };
        let midi = &entry.file;

        let (total_notes, polyphony, nps) = Self::compute_midi_stats(midi, midi_time);

        let stats = counter_stats.entry(clip_id).or_default();
        stats.total_notes = total_notes;
        stats.max_nps = stats.max_nps.max(nps);
        stats.max_polyphony = stats.max_polyphony.max(polyphony);
        stats.last_midi_time = midi_time;

        (stats.clone(), polyphony, nps)
    }

    fn render_counter_layers(
        &mut self,
        ctx: &mut LayerRenderContext<'_>,
        counter_clips: &[LayerData],
        make_rect: &impl Fn(&LayerCommon, f32, f32) -> (f32, f32, f32, f32),
        total_instances: usize,
        fps: u32,
        project: &mut ProjectState,
    ) {
        let rw = ctx.render_width as f32;
        let rh = ctx.render_height as f32;

        for counter in counter_clips {
            let midi_time = (ctx.time - counter.song_start_time).max(0.0) as f64;
            let (stats, current_polyphony, current_nps) =
                Self::update_counter_stats(&mut self.counter_stats, project, counter, midi_time);

            let Some(midi_idx) = counter.midi_idx else {
                continue;
            };
            let Some(entry) = project.midi.entries.get(midi_idx) else {
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

            if !self.text_layer_cache.contains_key(&counter.clip_id) {
                let layer = nezha_text::TextLayer::new(
                    &self.font_atlas,
                    self.render_ctx.device(),
                    self.render_ctx.queue(),
                    self.render_ctx.target_format(),
                );
                self.text_layer_cache.insert(counter.clip_id, layer);
            }
            let text_layer = self.text_layer_cache.get_mut(&counter.clip_id).unwrap();
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

            let atlas = &mut self.font_atlas;
            prepare_text(text_layer, atlas);

            let rect = make_rect(&counter.common, rw, rh);
            let encoder = self.render_ctx.encoder_mut();
            nezha_compositor::render_layer(
                encoder,
                text_layer,
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

    fn render_all_layers(
        &mut self,
        time: f32,
        render_width: u32,
        render_height: u32,
        project: &mut ProjectState,
    ) {
        let layers =
            preview_layer::collect_visible_layers(&project.timeline_state.data.tracks, time);
        let default_style = Self::default_style();
        let fps = project.timeline_state.fps;

        let preview_view = self.render_ctx.preview_view().clone();
        let mut ctx = LayerRenderContext {
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
                    Self::render_solid_color_layer(
                        &mut self.render_ctx,
                        &mut ctx,
                        clip,
                        rect,
                        is_first,
                    );
                    is_first = false;
                }
                ClipKind::Waterfall => {
                    let note_count = Self::render_waterfall_layer(
                        &mut self.render_ctx,
                        project,
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
                ClipKind::Image | ClipKind::Video => {
                    let cache = if clip.kind == ClipKind::Image {
                        &mut self.image_layer_cache
                    } else {
                        &mut self.video_layer_cache
                    };
                    Self::render_media_clip(
                        &mut self.render_ctx,
                        cache,
                        project,
                        &mut ctx,
                        clip,
                        rect,
                        is_first,
                    );
                    is_first = false;
                }
            }
        }

        if is_first {
            self.render_ctx.with_solid_color_layer([0.0, 0.0, 0.0, 1.0], |solid, encoder| {
                nezha_compositor::render_layer(
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

        self.render_counter_layers(&mut ctx, &counter_clips, &make_rect, total_instances, fps, project);
    }
}
