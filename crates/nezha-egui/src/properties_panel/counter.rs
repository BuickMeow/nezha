//! 计数器图层的属性面板 — 特有属性。

use crate::transport::TrackClip;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, clip: &mut TrackClip, midi_files: &[crate::app::project_state::MidiEntry]) {
    ui.heading("计数器");
    ui.add_space(2.0);

    // ── MIDI 选择 ──
    ui.label("关联 MIDI");
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
                .unwrap_or_else(|| "未选择".to_string()),
        )
        .show_ui(ui, |ui| {
            for (idx, name) in midi_names.iter().enumerate() {
                if ui.selectable_label(clip.midi_idx == Some(idx), name).clicked() {
                    clip.midi_idx = Some(idx);
                }
            }
            if ui.selectable_label(clip.midi_idx.is_none(), "无").clicked() {
                clip.midi_idx = None;
            }
        });

    ui.add_space(8.0);

    // ── 模板文本 ──
    ui.label("模板文本");
    ui.add(
        egui::TextEdit::multiline(&mut clip.template_text)
            .desired_rows(4)
            .desired_width(ui.available_width()),
    );

    // 占位符提示
    ui.collapsing("可用占位符", |ui| {
        ui.label(
            egui::RichText::new(
                "{nc} 累计音符  {nr} 剩余音符  {tn} 总音符  {tin} 同屏音符\n\
                 {nps} NPS  {mnps} 最大NPS  {plph} 复音  {mplph} 最大复音\n\
                 {bpm} BPM  {ppq} PPQ  {tsn}/{tsd} 拍号  {avgnps} 平均NPS\n\
                 {currtime} 当前时间  {totaltime} 总时长  {remtime} 剩余时间\n\
                 {currticks} 当前Tick  {totalticks} 总Tick  {remticks} 剩余Tick\n\
                 {currbars} 当前小节  {totalbars} 总小节  {rembars} 剩余小节\n\
                 {currframes} 当前帧  {totalframes} 总帧  {remframes} 剩余帧\n\
                 {notep} 音符进度%  {tickp} Tick进度%  {timep} 时间进度%",
            )
            .size(10.0)
            .color(ui.visuals().weak_text_color()),
        );
    });

    ui.add_space(8.0);

    // ── 对齐方式 ──
    ui.label("对齐方式");
    let mut align_idx = match clip.text_alignment {
        nezha_text::TextAlignment::TopLeft => 0usize,
        nezha_text::TextAlignment::TopRight => 1,
        nezha_text::TextAlignment::BottomLeft => 2,
        nezha_text::TextAlignment::BottomRight => 3,
    };
    let aligns = ["左上", "右上", "左下", "右下"];
    egui::ComboBox::from_id_salt("counter_align")
        .width(120.0)
        .selected_text(aligns[align_idx])
        .show_ui(ui, |ui| {
            for (i, &name) in aligns.iter().enumerate() {
                if ui.selectable_label(align_idx == i, name).clicked() {
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
    ui.label("字号");
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(&mut clip.font_size, 8..=128)
                .step_by(1.0)
                .text("px"),
        );
    });
    ui.label(
        egui::RichText::new(format!("当前: {} px", clip.font_size))
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );

    ui.add_space(8.0);

    // ── 文字颜色 ──
    ui.label("文字颜色");
    let mut rgb = [
        clip.text_color.r(),
        clip.text_color.g(),
        clip.text_color.b(),
    ];
    ui.color_edit_button_srgb(&mut rgb);
    clip.text_color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);

    ui.add_space(8.0);

    // ── 千位分隔符 ──
    ui.label("千位分隔符");
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut clip.thousand_separator,
            nezha_text::Separator::Comma,
            "逗号",
        );
        ui.selectable_value(
            &mut clip.thousand_separator,
            nezha_text::Separator::Dot,
            "点",
        );
        ui.selectable_value(
            &mut clip.thousand_separator,
            nezha_text::Separator::Nothing,
            "无",
        );
    });

    ui.add_space(4.0);

    // ── 零填充 ──
    ui.checkbox(&mut clip.zero_padding,
        "启用零填充",
    );

    ui.add_space(8.0);

    // ── 粗体 ──
    ui.horizontal(|ui| {
        ui.checkbox(&mut clip.bold, "粗体");
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
        ui.checkbox(&mut clip.italic, "斜体");
        if clip.italic {
            ui.add(
                egui::Slider::new(&mut clip.italic_slant, -1.0..=1.0)
                    .step_by(0.05)
                    .text("倾斜"),
            );
        }
    });

    ui.add_space(4.0);

    // ── 描边 ──
    ui.horizontal(|ui| {
        ui.checkbox(&mut clip.outline_enabled, "描边");
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
            ui.label("描边颜色");
            let mut outline_rgb = [
                clip.outline_color.r(),
                clip.outline_color.g(),
                clip.outline_color.b(),
            ];
            ui.color_edit_button_srgb(&mut outline_rgb);
            clip.outline_color = egui::Color32::from_rgb(outline_rgb[0], outline_rgb[1], outline_rgb[2]);
        });
    }

    ui.add_space(8.0);

    // ── 字间距 ──
    ui.label("字间距");
    ui.add(
        egui::Slider::new(&mut clip.letter_spacing, -5.0..=20.0)
            .step_by(0.5)
            .text("px"),
    );

    ui.add_space(4.0);

    // ── 最小字宽 ──
    ui.label("最小字宽");
    ui.add(
        egui::Slider::new(&mut clip.min_advance, 0.0..=20.0)
            .step_by(0.5)
            .text("px"),
    );

    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("计数器会统计关联 MIDI 的实时数据并应用模板。\n位置与合成方式在上方「变换 / 合成」中配置。")
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );
}
