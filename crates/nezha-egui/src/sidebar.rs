use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SidebarTab {
    #[default]
    Style,
    Project,
    Export,
    Settings,
}

pub fn show(ui: &mut egui::Ui, active_tab: &mut SidebarTab) {
    ui.vertical_centered(|ui| {
        ui.add_space(12.0);
        ui.heading("🎹");
        ui.add_space(20.0);

        let tabs = [
            (SidebarTab::Style, "🎨", "样式"),
            (SidebarTab::Project, "🎵", "项目"),
            (SidebarTab::Export, "📤", "导出"),
            (SidebarTab::Settings, "⚙️", "设置"),
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
