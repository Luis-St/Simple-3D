//! Controls drawn in the theme's own style.

use super::*;
use egui::{Color32, CornerRadius, Stroke};

/// A panel header bar: the name on the left, the caller's content on the right. The whole bar's
/// response is returned so it can collapse on click.
pub fn panel_header(ui: &mut egui::Ui, name: &str, right: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    let full = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(full, metric::ROW), egui::Sense::click());
    ui.painter().rect_filled(rect, 0.0, token::SURFACE_2);
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0_f32, token::SURFACE_3));
    let inner = rect.shrink2(egui::vec2(metric::PANEL_PAD, 0.0));
    let mut child =
        ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.add(egui::Label::new(header_text(name)).selectable(false));
    child.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
    response
}

/// The collapse triangle, painted: down when open, right when closed.
pub fn twisty(painter: &egui::Painter, centre: egui::Pos2, open: bool, colour: Color32) {
    let r = 4.0;
    let points = if open {
        vec![
            egui::pos2(centre.x - r, centre.y - r * 0.5),
            egui::pos2(centre.x + r, centre.y - r * 0.5),
            egui::pos2(centre.x, centre.y + r * 0.7),
        ]
    } else {
        vec![
            egui::pos2(centre.x - r * 0.5, centre.y - r),
            egui::pos2(centre.x - r * 0.5, centre.y + r),
            egui::pos2(centre.x + r * 0.7, centre.y),
        ]
    };
    painter.add(egui::Shape::convex_polygon(points, colour, Stroke::NONE));
}

/// The colour chip identifying an axis field.
pub fn axis_colour(axis: usize) -> Color32 {
    match axis {
        0 => token::AXIS_X,
        1 => token::AXIS_Y,
        _ => token::AXIS_Z,
    }
}

/// The axis chip's width, wide enough to grab as the field's scrub grip.
pub const AXIS_CHIP_WIDTH: f32 = 6.0;

/// Paint the small axis chip, which senses drags as the field's scrub grip.
pub fn axis_chip(ui: &mut egui::Ui, id: egui::Id, axis: usize) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(AXIS_CHIP_WIDTH, metric::INPUT_ROW - 8.0), egui::Sense::hover());
    let response = ui.interact(rect, id, egui::Sense::drag());
    let colour = axis_colour(axis);
    let colour = if response.hovered() || response.dragged() { colour } else { colour.gamma_multiply(0.8) };
    ui.painter().rect_filled(rect, CornerRadius::same(1), colour);
    response
}

/// The chip before a dimension, one band per axis it spans (issue 110). A dimension along no axis
/// still takes the room, so fields stay in one column.
pub fn axes_chip(ui: &mut egui::Ui, axes: &[usize]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(AXIS_CHIP_WIDTH, metric::INPUT_ROW - 8.0), egui::Sense::hover());
    let band = rect.height() / axes.len().max(1) as f32;
    for (index, &axis) in axes.iter().enumerate() {
        let top = rect.top() + band * index as f32;
        let part = egui::Rect::from_min_max(egui::pos2(rect.left(), top), egui::pos2(rect.right(), top + band));
        ui.painter().rect_filled(part, CornerRadius::same(1), axis_colour(axis).gamma_multiply(0.8));
    }
}

/// An on/off control that reads as a control while off: egui's toggle is invisible at rest, so
/// off is drawn as an outlined chip and on is left as egui's accent tint. Returns the response
/// like `egui::Ui::toggle_value`.
pub fn toggle(ui: &mut egui::Ui, on: &mut bool, text: &str) -> egui::Response {
    let mut response = chip(ui, *on, text);
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response
}

/// One option of an exclusive set, drawn like [`toggle`]. The caller compares against the current
/// choice, since reselecting is not an edit.
pub fn choice(ui: &mut egui::Ui, chosen: bool, text: &str) -> egui::Response {
    chip(ui, chosen, text)
}

/// The chip both are drawn as. `frame_when_inactive` keeps an unselected one looking like a
/// control; the on state is egui's own selected button.
pub(crate) fn chip(ui: &mut egui::Ui, on: bool, text: &str) -> egui::Response {
    let mut button = egui::Button::selectable(on, text).frame_when_inactive(true);
    if !on {
        // The divider grey: enough to read as a control without competing with an on chip.
        button = button.stroke(Stroke::new(1.0_f32, token::SURFACE_3));
    }
    ui.add(button)
}
