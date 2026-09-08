//! Drawing a popup where it was left, and the room its body has.

use super::*;
use crate::theme::{self, token};

/// Draw one in-place popup over `bounds`, and return whatever the user did to
/// the window itself.
///
/// `bounds` is the rectangle the window may be dragged around in -- the
/// viewport. `contents` is given the room inside the frame, already padded and
/// already the right width, and draws the whole of what is in the window: the
/// body, and then [`action_row`] for the buttons along its foot. One closure
/// rather than two, because both halves want the tool they belong to and a
/// second closure holding it as well is a second borrow of it.
pub fn show(
    ctx: &egui::Context,
    bounds: egui::Rect,
    placement: &mut Placement,
    spec: PopupSpec<'_>,
    contents: impl FnOnce(&mut egui::Ui),
) -> PopupEvent {
    let pos = settle(placement, spec.width, bounds);
    placement.pos = Some(pos);

    let mut event = PopupEvent::Nothing;
    // No `constrain_to`: the clamp above is the constraint, and two of them
    // disagree by a frame. egui's would move the area without writing the move
    // back into the placement, which is exactly the disagreement that made a
    // rolled-up window jump.
    let area = egui::Area::new(egui::Id::new(("in-place-popup", spec.key)))
        .order(egui::Order::Foreground)
        .movable(false)
        .fixed_pos(pos);

    let frame = egui::Frame::NONE
        .fill(token::SURFACE_1)
        .stroke(egui::Stroke::new(1.0_f32, token::SURFACE_3))
        .corner_radius(6.0)
        .shadow(egui::epaint::Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: egui::Color32::from_black_alpha(90),
        });

    let response = area.show(ctx, |ui| {
        frame.show(ui, |ui| {
            ui.set_width(spec.width);
            let drag = title_bar(ui, spec.key, spec.title, &mut placement.collapsed, &mut event);
            if drag != egui::Vec2::ZERO {
                let moved = clamp_into(pos + drag, egui::vec2(spec.width, TITLE_BAR), bounds);
                placement.pos = Some(moved);
            }
            if placement.collapsed {
                return;
            }
            ui.add_space(PAD - 4.0);
            egui::Frame::NONE
                .inner_margin(egui::Margin { left: PAD as i8, right: PAD as i8, top: 0, bottom: PAD as i8 })
                .show(ui, |ui| {
                    ui.set_width(spec.width - PAD * 2.0);
                    contents(ui);
                });
        });
    });
    // What it actually came out at, for the next frame's clamp.
    if !placement.collapsed {
        placement.height = response.response.rect.height();
    }
    event
}

/// How tall a popup's body may be before it has to scroll: the room in
/// `bounds` -- the viewport -- less the window's own chrome, which is the title
/// bar, the padding and the action row along the foot.
///
/// A popup is as tall as what is in it, which is the right answer until what is
/// in it is taller than the viewport: then the foot of the window goes off the
/// bottom of the screen, and the buttons that finish the job go with it. The
/// body scrolls instead, and the buttons stay where they are.
pub fn body_room(bounds: egui::Rect) -> f32 {
    (bounds.height() - TITLE_BAR - PAD * 3.0 - theme::metric::DIALOG_BUTTON - theme::metric::GAP * 2.0).max(120.0)
}

/// The buttons along the foot of a popup, under a rule: right-aligned and laid
/// out right to left, so the closure names the rightmost -- the one that goes
/// through with the command -- first. The same shape as a dialog's row, because
/// it is the same row.
pub fn action_row(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(PAD - 4.0);
    ui.separator();
    ui.add_space(2.0);
    // The room is measured out rather than left to the layout. A popup lives in
    // an `Area`, whose height is the screen below it until something says
    // otherwise, and a right-to-left layout handed that much took all of it:
    // the window came out the full height of the viewport with its buttons
    // pinned to the bottom of the screen and half a page of nothing above them.
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), theme::metric::DIALOG_BUTTON),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = theme::metric::GAP * 2.0;
            contents(ui);
        },
    );
}
