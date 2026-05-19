use super::App;
use crate::piano_view;
use crate::transport::ClipKind;
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

    pub(super) fn render_preview(&mut self, ui: &mut egui::Ui) {
        self.update_playback();

        let available = ui.available_size();
        let render_width = self.project.render.width;
        let render_height = self.project.render.height;
        let aspect = render_width as f32 / render_height as f32;
        let current_time = self.project.playback.current_time as f32;

        let layers = self.collect_visible_layers(current_time);
        let default_style = self.default_style();

        let mut is_first = true;
        for clip in &layers {
            let clear_background = is_first;
            is_first = false;

            // 每个 clip 独立 begin/end pass，
            // 确保 queue.write_buffer 的资源更新在对应 render pass 之前执行，
            // 避免多个 clip 共用 buffer 导致数据相互覆盖。
            self.render_ctx.begin_pass();

            match clip.kind {
                ClipKind::SolidColor => {
                    let style = nezha_renderer::RenderStyle {
                        background: [
                            clip.color.r() as f64 / 255.0,
                            clip.color.g() as f64 / 255.0,
                            clip.color.b() as f64 / 255.0,
                            1.0,
                        ],
                        ..default_style.clone()
                    };
                    self.render_ctx.render_background(
                        render_width,
                        render_height,
                        &style,
                        clear_background,
                    );
                }
                ClipKind::Waterfall => {
                    let Some(midi_idx) = clip.midi_idx else {
                        self.render_ctx.end_pass();
                        continue;
                    };
                    let Some(entry) = self.project.midi.entries.get(midi_idx) else {
                        self.render_ctx.end_pass();
                        continue;
                    };

                    let clip_time = (current_time - clip.clip_start).max(0.0) as f64;
                    let keyboard_height_px = render_height as f32 * clip.keyboard_height_percent;
                    let clip_style = nezha_renderer::RenderStyle {
                        render_mode: clip.render_mode,
                        border_width: clip.border_width,
                        rounding: clip.rounding,
                        track_index: 0,
                        palette: default_style.palette,
                        background: [0.0, 0.0, 0.0, 0.0], // 透明背景，空隙处可见下层
                        equal_key_width: clip.equal_key_width,
                        keyboard_height: keyboard_height_px,
                    };

                    self.render_ctx.render_layer(
                        render_width,
                        render_height,
                        clip_time,
                        clip.speed,
                        midi_idx,
                        &entry.file,
                        &clip_style,
                        clear_background,
                    );
                }
            }

            self.render_ctx.end_pass();
        }

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
