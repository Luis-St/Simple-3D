//! A panel's header, which is also its drag handle.

use super::*;
use crate::app::App;
use crate::theme::{self, metric, token};
use simple3d_core::config::{Panel, Side};

/// The header bar: the panel's name, a twisty that says whether it is rolled up,
/// and the grip the whole bar is. Returns its vertical centre.
pub(crate) fn header(app: &mut App, ui: &mut egui::Ui, panel: Panel, side: Side) -> f32 {
    let collapsed = app.settings.layout.is_collapsed(panel);
    let full = ui.available_width();
    // The bar's id names the panel rather than being taken from where the bar
    // happens to sit, so a header keeps its identity across a move -- and so a
    // test can find the bar it means to drag.
    let (rect, _) = ui.allocate_exact_size(egui::vec2(full, metric::ROW), egui::Sense::hover());
    let response = ui.interact(rect, header_id(panel), egui::Sense::click_and_drag());
    let dragging = app.dock_drag.panel == Some(panel);
    let fill = if dragging || response.hovered() { token::SURFACE_3 } else { token::SURFACE_2 };
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0_f32, token::SURFACE_3));
    theme::twisty(ui.painter(), egui::pos2(rect.left() + 11.0, rect.center().y), !collapsed, token::TEXT_LO);
    ui.painter().text(
        egui::pos2(rect.left() + 22.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        theme::header_text(panel.label()).text(),
        egui::FontId::proportional(theme::font::HEADER),
        token::TEXT_LO,
    );
    // The grip dots on the right say the bar can be dragged, in the one place a
    // pointer would go looking for them.
    for i in 0..3 {
        let x = rect.right() - 10.0;
        let y = rect.center().y - 4.0 + i as f32 * 4.0;
        ui.painter().hline(x - 5.0..=x, y, egui::Stroke::new(1.0_f32, token::TEXT_LO.gamma_multiply(0.6)));
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if response.clicked() {
        app.settings.layout.toggle_collapsed(panel);
    }
    if response.drag_started() {
        app.dock_drag.panel = Some(panel);
    }
    let _ = side;
    rect.center().y
}
