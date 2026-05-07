use eframe::egui;
use crate::transport::TimelineState;

pub fn show(ui: &mut egui::Ui, timeline_state: &mut TimelineState) {
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
}
