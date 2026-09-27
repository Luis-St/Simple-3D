//! Drawing a popup where it was left, and the room its body has.

use super::*;
use crate::theme::{self, token};

/// Draw one in-place popup within `bounds` (the viewport) and return what was done to its window.
/// `contents` gets the padded inner room and draws the body and then [`action_row`]; one closure,
/// since both halves borrow the same tool.
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
    // No `constrain_to`: the clamp above is the constraint, and egui's would move the area without
    // updating the placement, which made rolled-up windows jump.
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
    // The actual height, for the next frame's clamp.
    if !placement.collapsed {
        let height = response.response.rect.height();
        placement.height = height;
        // And how much of it was not the body, for the next frame's room.
        let layer = response.response.layer_id.id;
        if let Some(body) = ctx.data(|d| d.get_temp::<f32>(layer.with(BODY_HEIGHT))) {
            ctx.data_mut(|d| d.insert_temp(layer.with(CHROME_HEIGHT), height - body));
        }
    }
    event
}

/// Where [`scrolling_body`] and [`show`] store the body and chrome heights, under the popup's layer.
const BODY_HEIGHT: &str = "popup-body-height";
const CHROME_HEIGHT: &str = "popup-chrome-height";

/// How tall a popup's body may be before scrolling: the viewport height less the measured chrome,
/// so the action buttons never go off screen. A constant sum is used only on the first frame,
/// since it missed egui's spacing.
fn body_room(ui: &egui::Ui, bounds: egui::Rect) -> f32 {
    let estimate = TITLE_BAR + PAD * 3.0 + theme::metric::DIALOG_BUTTON + theme::metric::GAP * 2.0;
    let chrome = ui.ctx().data(|d| d.get_temp::<f32>(ui.layer_id().id.with(CHROME_HEIGHT))).unwrap_or(estimate);
    (bounds.height() - chrome).max(120.0)
}

/// A popup's body, scrolling once taller than [`body_room`]. The scrollbar sits in the window's
/// right padding so it does not squeeze the rows.
pub fn scrolling_body(ui: &mut egui::Ui, bounds: egui::Rect, body: impl FnOnce(&mut egui::Ui)) {
    let width = ui.available_width();
    let max_height = body_room(ui, bounds);
    let mut room = ui.available_rect_before_wrap();
    room.max.x += PAD;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(room));
    let (area, restore) = theme::list_scroll_area(&mut child);
    // Gap, bar and margin exactly fill the padding.
    let scroll = &mut child.style_mut().spacing.scroll;
    scroll.bar_inner_margin = 2.0;
    scroll.bar_width = PAD - 4.0;
    scroll.bar_outer_margin = 2.0;
    area.auto_shrink([false, true]).max_height(max_height).show(&mut child, |ui| {
        ui.set_style(restore);
        // Never wider than given, since the room briefly shrinks while the bar slides in.
        ui.set_max_width(ui.available_width().min(width));
        body(ui);
    });
    // Only the column is claimed; the bar's padding already belongs to the window.
    let used = child.min_rect();
    ui.ctx().data_mut(|d| d.insert_temp(ui.layer_id().id.with(BODY_HEIGHT), used.height()));
    ui.advance_cursor_after_rect(egui::Rect::from_min_size(used.min, egui::vec2(width, used.height())));
}

/// The popup's footer buttons under a rule, right to left so the closure names the confirming
/// button first; the same row as a dialog's.
pub fn action_row(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(PAD - 4.0);
    ui.separator();
    ui.add_space(2.0);
    // The room is measured explicitly: an `Area` is as tall as the screen below it, and a
    // right-to-left layout took all of it.
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), theme::metric::DIALOG_BUTTON),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = theme::metric::GAP * 2.0;
            contents(ui);
        },
    );
}
