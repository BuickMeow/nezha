//! 计数器图层的属性面板 — 特有属性。

use crate::transport::TrackClip;
use eframe::egui;
use rust_i18n::t;

pub fn show(
    ui: &mut egui::Ui,
    clip: &mut TrackClip,
    midi_files: &[crate::app::project_state::MidiEntry],
) {
    ui.heading(t!("counter.title"));
    ui.add_space(2.0);

    // ── MIDI 选择 ──
    ui.label(t!("counter.midi"));
    let midi_names: Vec<String> = midi_files
        .iter()
        .map(|e| {
            std::path::Path::new(&e.path)
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("MIDI")
                .to_string()
        })
        .collect();

    egui::ComboBox::from_id_salt("counter_midi_select")
        .width(ui.available_width())
        .selected_text(
            clip.midi_idx
                .and_then(|idx| midi_names.get(idx))
                .cloned()
                .unwrap_or_else(|| t!("counter.midi.none").to_string()),
        )
        .show_ui(ui, |ui| {
            for (idx, name) in midi_names.iter().enumerate() {
                if ui
                    .selectable_label(clip.midi_idx == Some(idx), name)
                    .clicked()
                {
                    clip.midi_idx = Some(idx);
                }
            }
            if ui.selectable_label(clip.midi_idx.is_none(), t!("counter.none")).clicked() {
                clip.midi_idx = None;
            }
        });

    ui.add_space(8.0);

    // ── 模板文本 ──
    ui.label(t!("counter.template"));
    ui.add(
        egui::TextEdit::multiline(&mut clip.template_text)
            .desired_rows(4)
            .desired_width(ui.available_width()),
    );

    // 占位符提示
    ui.collapsing(t!("counter.placeholders"), |ui| {
        ui.label(
            egui::RichText::new(t!("counter.placeholders.content"))
                .size(10.0)
                .color(ui.visuals().weak_text_color()),
        );
    });

    ui.add_space(8.0);

    // ── 对齐方式 ──
    ui.label(t!("counter.alignment"));
    let mut align_idx = match clip.text_alignment {
        nezha_text::TextAlignment::TopLeft => 0usize,
        nezha_text::TextAlignment::TopRight => 1,
        nezha_text::TextAlignment::BottomLeft => 2,
        nezha_text::TextAlignment::BottomRight => 3,
    };
    let aligns = [
        t!("counter.alignment.top_left").to_string(),
        t!("counter.alignment.top_right").to_string(),
        t!("counter.alignment.bottom_left").to_string(),
        t!("counter.alignment.bottom_right").to_string(),
    ];
    egui::ComboBox::from_id_salt("counter_align")
        .width(120.0)
        .selected_text(&aligns[align_idx])
        .show_ui(ui, |ui| {
            for (i, name) in aligns.iter().enumerate() {
                if ui.selectable_label(align_idx == i, name.as_str()).clicked() {
                    align_idx = i;
                }
            }
        });
    clip.text_alignment = match align_idx {
        1 => nezha_text::TextAlignment::TopRight,
        2 => nezha_text::TextAlignment::BottomLeft,
        3 => nezha_text::TextAlignment::BottomRight,
        _ => nezha_text::TextAlignment::TopLeft,
    };

    ui.add_space(8.0);

    // ── 字体 ──
    ui.label(t!("counter.font_size"));
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(&mut clip.font_size, 8..=128)
                .step_by(1.0)
                .text("px"),
        );
    });
    ui.label(
        egui::RichText::new(t!("counter.font_size.current", size = clip.font_size))
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );

    ui.add_space(8.0);

    // ── 文字颜色 ──
    ui.label(t!("counter.text_color"));
    let mut rgb = [
        clip.text_color.r(),
        clip.text_color.g(),
        clip.text_color.b(),
    ];
    ui.color_edit_button_srgb(&mut rgb);
    clip.text_color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);

    ui.add_space(8.0);

    // ── 千位分隔符 ──
    ui.label(t!("counter.thousand_separator"));
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut clip.thousand_separator,
            nezha_text::Separator::Comma,
            t!("counter.thousand_separator.comma"),
        );
        ui.selectable_value(
            &mut clip.thousand_separator,
            nezha_text::Separator::Dot,
            t!("counter.thousand_separator.dot"),
        );
        ui.selectable_value(
            &mut clip.thousand_separator,
            nezha_text::Separator::Nothing,
            t!("counter.thousand_separator.none"),
        );
    });

    ui.add_space(4.0);

    // ── 零填充 ──
    ui.checkbox(&mut clip.zero_padding, t!("counter.zero_padding"));

    ui.add_space(8.0);

    // ── 粗体 ──
    ui.horizontal(|ui| {
        ui.checkbox(&mut clip.bold, t!("counter.bold"));
        if clip.bold {
            ui.add(
                egui::Slider::new(&mut clip.bold_offset, 0.5..=4.0)
                    .step_by(0.1)
                    .text("px"),
            );
        }
    });

    ui.add_space(4.0);

    // ── 斜体 ──
    ui.horizontal(|ui| {
        ui.checkbox(&mut clip.italic, t!("counter.italic"));
        if clip.italic {
            ui.add(
                egui::Slider::new(&mut clip.italic_slant, -1.0..=1.0)
                    .step_by(0.05)
                    .text(t!("counter.italic_slant")),
            );
        }
    });

    ui.add_space(4.0);

    // ── 描边 ──
    ui.horizontal(|ui| {
        ui.checkbox(&mut clip.outline_enabled, t!("counter.outline"));
        if clip.outline_enabled {
            ui.add(
                egui::Slider::new(&mut clip.outline_width, 0.5..=8.0)
                    .step_by(0.1)
                    .text("px"),
            );
        }
    });
    if clip.outline_enabled {
        ui.horizontal(|ui| {
            ui.label(t!("counter.outline.color"));
            let mut outline_rgb = [
                clip.outline_color.r(),
                clip.outline_color.g(),
                clip.outline_color.b(),
            ];
            ui.color_edit_button_srgb(&mut outline_rgb);
            clip.outline_color =
                egui::Color32::from_rgb(outline_rgb[0], outline_rgb[1], outline_rgb[2]);
        });
    }

    ui.add_space(8.0);

    // ── 字间距 ──
    ui.label(t!("counter.letter_spacing"));
    ui.add(
        egui::Slider::new(&mut clip.letter_spacing, -5.0..=20.0)
            .step_by(0.5)
            .text("px"),
    );

    ui.add_space(4.0);

    // ── 最小字宽 ──
    ui.label(t!("counter.min_advance"));
    ui.add(
        egui::Slider::new(&mut clip.min_advance, 0.0..=20.0)
            .step_by(0.5)
            .text("px"),
    );

    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(t!("counter.hint"))
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );
}
