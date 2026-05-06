use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarTab {
    #[default]
    Midi,
    Style,
    Export,
}

pub fn show(ui: &mut egui::Ui, active_tab: &mut SidebarTab) {
    ui.vertical_centered(|ui| {
        ui.add_space(12.0);
        ui.heading("🎹");
        ui.add_space(20.0);

        let tabs = [
            (SidebarTab::Midi, "🎵", "MIDI"),
            (SidebarTab::Style, "🎨", "样式"),
            (SidebarTab::Export, "📤", "导出"),
        ];

        for (tab, icon, label) in tabs {
            let selected = *active_tab == tab;
            let response = ui.selectable_label(selected, format!("{}\n{}", icon, label));
            if response.clicked() {
                *active_tab = tab;
            }
            ui.add_space(8.0);
        }
    });
}
