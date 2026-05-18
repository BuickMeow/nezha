use super::App;
use crate::piano_view;
use crate::transport::{ClipKind, TrackClip};
use eframe::egui;

/// 图层渲染所需数据（复制自 TrackClip，避免持有 self 的引用）。
#[derive(Clone)]
struct LayerData {
    kind: ClipKind,
    midi_idx: Option<usize>,
    speed: f32,
    border_width: f32,
    rounding: f32,
    render_mode: nezha_renderer::RenderMode,
    equal_key_width: bool,
    clip_start: f32,
    color: egui::Color32,
}

impl App {
    /// 收集当前时间点所有可见图层数据（Premiere 顺序：底 -> 顶）。
    fn collect_visible_layers(&self, time: f32) -> Vec<LayerData> {
        let mut layers = Vec::new();
        for track in self.project.timeline_state.data.tracks.iter().rev() {
            for clip in &track.clips {
                if time >= clip.start && time < clip.end {
                    layers.push(LayerData {
                        kind: clip.kind,
                        midi_idx: clip.midi_idx,
                        speed: clip.speed,
                        border_width: clip.border_width,
                        rounding: clip.rounding,
                        render_mode: clip.render_mode,
                        equal_key_width: clip.equal_key_width,
                        clip_start: clip.start,
                        color: clip.color,
                    });
                }
            }
        }
        layers
    }

    fn default_style(&self) -> nezha_renderer::RenderStyle {
        let timeline_state = &self.project.timeline_state;
        let clip = timeline_state.selected_clip();
        let (border_width, rounding, track_index, render_mode, equal_key_width, keyboard_percent) =
            clip.map(|clip| {
                (
                    clip.border_width,
                    clip.rounding,
                    clip.id,
                    clip.render_mode,
                    clip.equal_key_width,
                    clip.keyboard_height_percent,
                )
            })
            .unwrap_or(TrackClip::default_render_params());

        let keyboard_height_px = self.project.render.height as f32 * keyboard_percent;

        nezha_renderer::RenderStyle {
            render_mode,
            border_width,
            rounding,
            track_index,
            palette: nezha_renderer::random_palette(),
            background: [0.0, 0.0, 0.0, 1.0],
            equal_key_width,
            keyboard_height: keyboard_height_px,
        }
    }

    pub(super) fn render_preview(&mut self, ui: &mut egui::Ui) {
        self.update_playback();

        let available = ui.available_size();
        let render_width = self.project.render.width;
        let render_height = self.project.render.height;
        let aspect = render_width as f32 / render_height as f32;
        let current_time = self.project.playback.current_time as f32;

        let layers = self.collect_visible_layers(current_time);
        let default_style = self.default_style();

        let background_clip = layers.iter().find(|clip| clip.kind == ClipKind::SolidColor);
        let background_style = if let Some(background_clip) = background_clip {
            nezha_renderer::RenderStyle {
                background: [
                    background_clip.color.r() as f64 / 255.0,
                    background_clip.color.g() as f64 / 255.0,
                    background_clip.color.b() as f64 / 255.0,
                    1.0,
                ],
                ..default_style.clone()
            }
        } else {
            default_style.clone()
        };

        self.render_ctx.begin_pass();
        self.render_ctx
            .render_background(render_width, render_height, &background_style);

        let mut is_first_waterfall = true;
        for clip in &layers {
            if clip.kind != ClipKind::Waterfall {
                continue;
            }

            let Some(midi_idx) = clip.midi_idx else {
                continue;
            };
            let Some(entry) = self.project.midi.entries.get(midi_idx) else {
                continue;
            };

            let clip_time = (current_time - clip.clip_start).max(0.0) as f64;
            let clip_style = nezha_renderer::RenderStyle {
                render_mode: clip.render_mode,
                border_width: clip.border_width,
                rounding: clip.rounding,
                track_index: 0,
                palette: default_style.palette,
                background: default_style.background,
                equal_key_width: clip.equal_key_width,
                keyboard_height: default_style.keyboard_height,
            };

            self.render_ctx.render_layer(
                render_width,
                render_height,
                clip_time,
                clip.speed,
                midi_idx,
                &entry.file,
                &clip_style,
                is_first_waterfall && background_clip.is_none(),
            );
            is_first_waterfall = false;
        }
        self.render_ctx.end_pass();

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
