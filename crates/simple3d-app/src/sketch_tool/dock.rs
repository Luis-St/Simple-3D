//! The editor's panel in the right dock: while sketching it takes the place of whatever panels the
//! dock holds, so the tools sit where the properties do, and gives them back on leaving.

use crate::app::App;
use crate::theme::{self, metric, token};

/// Whether the panel is in the right dock: while sketching with the docks shown.
pub(crate) fn in_dock(app: &App) -> bool {
    app.sketch_tool.is_some() && !app.settings.layout.docks_hidden
}

/// The panel filling the dock: its title, the tool in use pinned under it, the scrolling body and
/// the buttons pinned to the bottom.
pub(crate) fn dock_panel(app: &mut App, ui: &mut egui::Ui) {
    let Some(tool) = app.sketch_tool.as_ref() else { return };
    let title = super::window::title(tool);
    theme::panel_header(ui, title, |_| {});
    let pad = metric::PANEL_PAD as i8;
    let hint = egui::Frame::NONE.fill(token::SURFACE_1).inner_margin(egui::Margin {
        left: pad,
        right: pad,
        top: 6,
        bottom: 8,
    });
    egui::TopBottomPanel::top("sketch-dock-hint").frame(hint).show_inside(ui, |ui| super::window::hint_line(app, ui));
    let frame = egui::Frame::NONE.fill(token::SURFACE_1).inner_margin(egui::Margin {
        left: pad,
        right: pad,
        top: 0,
        bottom: 8,
    });
    egui::TopBottomPanel::bottom("sketch-dock-actions").frame(frame).show_inside(ui, |ui| {
        crate::popup::action_row(ui, |ui| super::window::actions(app, ui));
    });
    egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(ui, |ui| {
        let (area, restore) = theme::list_scroll_area(ui);
        area.id_salt("sketch-dock-body").show(ui, |ui| {
            ui.set_style(restore);
            super::window::body(app, ui);
        });
    });
}
