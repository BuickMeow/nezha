use eframe::egui;
use rust_i18n::t;

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
            (SidebarTab::Style, "🎨", t!("sidebar.style")),
            (SidebarTab::Project, "🎵", t!("sidebar.project")),
            (SidebarTab::Media, "📦", t!("sidebar.media")),
            (SidebarTab::Export, "📤", t!("sidebar.export")),
            (SidebarTab::Settings, "\u{2699}", t!("sidebar.settings")),
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
