//! The controls drawn in the theme's own style.

use super::*;
use egui::{Color32, CornerRadius, Stroke};

/// Draw a panel header bar: the name on the left, whatever the caller wants on
/// the right. Returns the response of the whole bar so it can be clicked to
/// collapse.
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

/// The collapse triangle, drawn rather than typed. Points down when the thing
/// it opens is open, right when it is closed -- the convention every tree in
/// every file manager uses, which is why it needs no label.
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

/// The colour chip that stands in front of an axis field, so a row of three
/// numbers says which is which without spelling out X, Y and Z.
pub fn axis_colour(axis: usize) -> Color32 {
    match axis {
        0 => token::AXIS_X,
        1 => token::AXIS_Y,
        _ => token::AXIS_Z,
    }
}

/// Width of the axis chip. Wide enough to be grabbed, since it doubles as the
/// scrub grip for the field behind it -- a 3 px target could not be.
pub const AXIS_CHIP_WIDTH: f32 = 6.0;

/// Paint the small axis chip. It senses a drag: it is the only label an axis
/// field has, so it is the label that scrubs it.
pub fn axis_chip(ui: &mut egui::Ui, id: egui::Id, axis: usize) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(AXIS_CHIP_WIDTH, metric::INPUT_ROW - 8.0), egui::Sense::hover());
    let response = ui.interact(rect, id, egui::Sense::drag());
    let colour = axis_colour(axis);
    let colour = if response.hovered() || response.dragged() { colour } else { colour.gamma_multiply(0.8) };
    ui.painter().rect_filled(rect, CornerRadius::same(1), colour);
    response
}

/// A control that is on or off, drawn so that it reads as a control while it is
/// off.
///
/// egui's own toggle paints nothing at rest: an unset one is a bare word sitting
/// on the panel, the same size and colour as the row labels around it, and the
/// only thing that says it can be clicked at all is a background that appears
/// once the pointer is already on it. A control that has to be found by sweeping
/// the panel with the mouse is not a control. So an off one is drawn as a chip
/// too -- the input-field fill, outlined in the divider grey -- and the only
/// question a row of them leaves is which ones are on. What an *on* one looks
/// like was never the problem, and is left as egui draws it: the accent tint.
///
/// Takes and returns the flag the way `egui::Ui::toggle_value` does, so the
/// response is `changed()` on the click that flipped it.
pub fn toggle(ui: &mut egui::Ui, on: &mut bool, text: &str) -> egui::Response {
    let mut response = chip(ui, *on, text);
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response
}

/// One option out of a set, exactly one of which is chosen: the same chip,
/// clicked to choose rather than to flip. The caller compares the click against
/// what is chosen now, since choosing what is already chosen is not an edit.
///
/// Deliberately the same shape as [`toggle`]: to the eye both are "a thing that
/// is on or off and can be clicked", and the difference between them -- whether
/// turning one on turns its neighbour off -- is what the row's label says.
pub fn choice(ui: &mut egui::Ui, chosen: bool, text: &str) -> egui::Response {
    chip(ui, chosen, text)
}

/// The chip both are drawn as. `frame_when_inactive` is the whole point: egui
/// turns it off for a selectable button, which is what leaves an unselected one
/// looking like a label.
///
/// On, this is exactly egui's own selected button -- the accent tint it has
/// always been, untouched. Only the off state is drawn differently, because only
/// the off state was invisible.
pub(crate) fn chip(ui: &mut egui::Ui, on: bool, text: &str) -> egui::Response {
    let mut button = egui::Button::selectable(on, text).frame_when_inactive(true);
    if !on {
        // The divider grey, the same line the panel draws between its sections:
        // enough to say "control" against the input-field fill behind it without
        // competing with the chip beside it that is actually on.
        button = button.stroke(Stroke::new(1.0_f32, token::SURFACE_3));
    }
    ui.add(button)
}
