//! The bar a popup is dragged and rolled up by.

use super::*;
use crate::theme::{self, token};

/// The bar along the top: the chevron that rolls the window up, the title, and
/// the cross that closes it. Returns how far it was dragged this frame.
///
/// The two buttons are identified by the popup's `key` rather than by its
/// title, because a title may name what the tool is working on and so change
/// under the pointer -- and a widget whose id changes is one that loses the
/// press it is halfway through.
pub(crate) fn title_bar(
    ui: &mut egui::Ui,
    key: &str,
    title: &str,
    collapsed: &mut bool,
    event: &mut PopupEvent,
) -> egui::Vec2 {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, TITLE_BAR), egui::Sense::drag());
    let painter = ui.painter_at(rect);
    // Only the top corners are rounded: the bar is the top of the window, not a
    // widget sitting inside it.
    painter.rect_filled(
        rect,
        egui::CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 },
        if response.dragged() { token::SURFACE_3 } else { token::SURFACE_2 },
    );
    painter.hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0_f32, token::SURFACE_3));
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }

    // The chevron rolls the window up, which is what a popup that is in the way
    // needs rather than being put away: what is typed into it survives.
    let chevron = egui::Rect::from_min_size(egui::pos2(rect.left() + 5.0, rect.top() + 6.0), egui::Vec2::splat(14.0));
    let roll = ui.interact(chevron, ui.id().with((key, "roll")), egui::Sense::click());
    theme::twisty(
        &painter,
        chevron.center(),
        !*collapsed,
        if roll.hovered() { token::TEXT_HI } else { token::TEXT_LO },
    );
    if roll.clicked() {
        *collapsed = !*collapsed;
    }
    roll.on_hover_text(if *collapsed { "Unroll" } else { "Roll up, keeping what is set" });

    // The cross closes it.
    let cross = egui::Rect::from_min_size(egui::pos2(rect.right() - 21.0, rect.top() + 6.0), egui::Vec2::splat(14.0));
    let close = ui.interact(cross, ui.id().with((key, "close")), egui::Sense::click());
    let colour = if close.hovered() { token::DANGER } else { token::TEXT_LO };
    let arm = cross.shrink(3.5);
    let stroke = egui::Stroke::new(1.4_f32, colour);
    painter.line_segment([arm.left_top(), arm.right_bottom()], stroke);
    painter.line_segment([arm.right_top(), arm.left_bottom()], stroke);
    if close.clicked() {
        *event = PopupEvent::Closed;
    }
    let _ = close.on_hover_text("Close");

    let galley = painter.layout_no_wrap(
        title.to_string(),
        egui::FontId::proportional(theme::font::VALUE),
        token::TEXT_HI.gamma_multiply(0.9),
    );
    let text = ui.painter_at(egui::Rect::from_min_max(
        egui::pos2(chevron.right() + 5.0, rect.top()),
        egui::pos2(cross.left() - 5.0, rect.bottom()),
    ));
    text.galley(egui::pos2(chevron.right() + 5.0, rect.center().y - galley.size().y / 2.0), galley, token::TEXT_HI);

    // Double-clicking the bar rolls it up too, which is the gesture every title
    // bar has had for thirty years.
    if response.double_clicked() {
        *collapsed = !*collapsed;
    }
    response.drag_delta()
}
