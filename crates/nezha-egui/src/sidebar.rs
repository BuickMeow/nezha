use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SidebarTab {
    #[default]
    Style,
    Project,
    Media,
    Export,
    Settings,
}

pub fn show(ui: &mut egui::Ui, active_tab: &mut SidebarTab, panel_visible: &mut bool) {
    ui.vertical_centered(|ui| {
        ui.add_space(12.0);
        ui.heading("🎹");
        ui.add_space(20.0);

        let tabs = [
            (SidebarTab::Style, "🎨", "样式"),
            (SidebarTab::Project, "🎵", "项目"),
            (SidebarTab::Media, "📦", "素材"),
            (SidebarTab::Export, "📤", "导出"),
            (SidebarTab::Settings, "\u{2699}", "设置"),
        ];

        for (tab, icon, label) in tabs {
            let selected = *active_tab == tab && *panel_visible;
            let response = ui.selectable_label(selected, format!("{}\n{}", icon, label));
            if response.clicked() {
                if *active_tab == tab && *panel_visible {
                    *panel_visible = false;
                } else {
                    *active_tab = tab;
                    *panel_visible = true;
                }
            }
            ui.add_space(8.0);
        }
    });
}
