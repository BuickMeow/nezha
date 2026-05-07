use eframe::egui;
use crate::transport::TimelineState;

pub struct PropsPanelStyle<'a> {
    pub border_width: &'a mut f32,
    pub rounding: &'a mut f32,
    pub palette: &'a mut [[f32; 3]; 16],
}

pub fn show(ui: &mut egui::Ui, timeline_state: &mut TimelineState, style: &mut PropsPanelStyle) {
    ui.heading("属性");
    ui.separator();

    if let Some(selected_id) = timeline_state.selected_clip_id {
        let mut found = false;
        for track in &mut timeline_state.data.tracks {
            for clip in &mut track.clips {
                if clip.id == selected_id {
                    found = true;

                    ui.label(egui::RichText::new(&clip.name).strong());
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("开始:");
                        ui.label(format!("{:.2}s", clip.start));
                    });
                    ui.horizontal(|ui| {
                        ui.label("结束:");
                        ui.label(format!("{:.2}s", clip.end));
                    });
                    ui.horizontal(|ui| {
                        ui.label("时长:");
                        ui.label(format!("{:.2}s", clip.end - clip.start));
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    ui.label("流速");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Slider::new(&mut clip.speed, 0.1..=5.0)
                                .step_by(0.1)
                                .text("x"),
                        );
                    });
                    ui.label(
                        egui::RichText::new(format!("当前: {:.1}x", clip.speed))
                            .size(11.0)
                            .color(ui.visuals().weak_text_color()),
                    );
                    break;
                }
            }
            if found { break; }
        }

        ui.add_space(8.0);
        if ui.button("取消选择").clicked() {
            timeline_state.selected_clip_id = None;
        }
    } else {
        ui.label("未选中任何图层");
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("在时间轴上点击一个片段\n以编辑其属性")
                .size(11.0)
                .color(ui.visuals().weak_text_color()),
        );
    }

    ui.add_space(8.0);
    ui.separator();
    ui.heading("瀑布流样式");
    ui.add_space(4.0);

    ui.label("边框宽度");
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(style.border_width, 0.0..=1.0)
                .step_by(0.05)
                .text(""),
        );
    });
    ui.label(
        egui::RichText::new(format!("{:.0}%", *style.border_width * 100.0))
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );

    ui.add_space(4.0);
    ui.label("圆角");
    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(style.rounding, 0.0..=1.0)
                .step_by(0.05)
                .text(""),
        );
    });
    ui.label(
        egui::RichText::new(format!("{:.0}%", *style.rounding * 100.0))
            .size(11.0)
            .color(ui.visuals().weak_text_color()),
    );

    ui.add_space(4.0);
    ui.label("调色板");
    egui::Grid::new("palette_grid").striped(true).show(ui, |ui| {
        for ch in 0..16 {
            let [r, g, b] = style.palette[ch];
            let col = egui::Color32::from_rgb(
                (r * 255.0) as u8,
                (g * 255.0) as u8,
                (b * 255.0) as u8,
            );
            ui.colored_label(col, "██");
            ui.label(format!("ch{}", ch));
            if (ch + 1) % 4 == 0 {
                ui.end_row();
            }
        }
    });
}
