//! An in-place popup: a small window that floats over the viewport rather than
//! standing in front of the whole application (issue 82).
//!
//! The kind of window a CAD tool puts a command's settings in -- Fusion 360's
//! command palettes are the shape this copies. Three things separate it from
//! the dialogs in [`crate::app_chrome`]:
//!
//! * **It is drawn inside the viewport and cannot leave it.** A dialog is a
//!   window of the window system's own, which can be dragged onto the other
//!   screen and left there; this belongs to the picture it is about.
//! * **It is moved by its title bar and rolled up by the chevron on it**, so
//!   the part of the model it is covering can be seen without putting the tool
//!   away and losing what was typed into it.
//! * **It is not modal.** The viewport underneath goes on orbiting, zooming and
//!   selecting while it is open, which is the whole reason a tool with a live
//!   preview is worth having in the viewport at all.
//!
//! Nothing here knows what a split is. A popup is a title, a body, a row of
//! buttons and a [`Placement`] the application keeps for it, so the next tool
//! that wants one asks for one.

use crate::theme::{self, token};

/// Where a popup sits and whether it is rolled up. Kept by the application, one
/// per popup, so a tool closed and opened again comes back where it was left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placement {
    /// The top-left corner, in screen points. `None` until the first frame,
    /// which puts it in the corner of the viewport it opens over.
    pub pos: Option<egui::Pos2>,
    /// Rolled up to its title bar alone.
    pub collapsed: bool,
    /// How tall the window came out last frame, which is what this frame keeps
    /// inside the viewport.
    ///
    /// One frame behind, and deliberately: a window's height is not known until
    /// it has been laid out, and the alternative -- letting the toolkit
    /// constrain the area itself -- moves the window without saying so. That
    /// was the bug: a tall window near the bottom edge was quietly lifted to
    /// fit, and rolling it up removed the reason for the lift, so the title bar
    /// dropped back down under the pointer that had just clicked it.
    height: f32,
}

pub struct PopupSpec<'a> {
    /// Identifies the popup to the toolkit and to the placement store. Stable
    /// across openings -- it is what remembers where the window was dragged to.
    pub key: &'static str,
    pub title: &'a str,
    /// How wide the window is. A popup is a column of fields, not a document:
    /// it has one width and takes whatever height its contents come to.
    pub width: f32,
}

/// What the user did to the window itself, as opposed to what they did in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupEvent {
    Nothing,
    /// The close cross was pressed. The caller decides what closing means --
    /// for a tool it is a cancel.
    Closed,
}

/// The height of the bar the window is dragged by.
const TITLE_BAR: f32 = 26.0;
/// The margin inside the window, around the body and the button row.
const PAD: f32 = 10.0;

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

/// The bar along the top: the chevron that rolls the window up, the title, and
/// the cross that closes it. Returns how far it was dragged this frame.
///
/// The two buttons are identified by the popup's `key` rather than by its
/// title, because a title may name what the tool is working on and so change
/// under the pointer -- and a widget whose id changes is one that loses the
/// press it is halfway through.
fn title_bar(ui: &mut egui::Ui, key: &str, title: &str, collapsed: &mut bool, event: &mut PopupEvent) -> egui::Vec2 {
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

/// Where the window comes to rest this frame: where it was left, brought inside
/// the viewport.
///
/// Opened in the top-left of the viewport, a comfortable margin in -- over the
/// corner of the picture rather than over the middle of it, which is where the
/// thing being worked on is.
///
/// Clamped every frame and not only when it is dragged, because the viewport
/// can be made smaller and a popup left off the edge of a shrunken one cannot
/// be reached to be dragged back. Clamped against the *whole* window rather
/// than its title bar alone, so the position that gets stored is one the window
/// already fits at -- rolling it up then loosens the clamp and moves nothing,
/// which is what a roll-up has to do: the bar stays exactly under the chevron
/// that was clicked.
fn settle(placement: &Placement, width: f32, bounds: egui::Rect) -> egui::Pos2 {
    let pos = placement.pos.unwrap_or_else(|| bounds.left_top() + egui::vec2(16.0, 16.0));
    let tall = if placement.collapsed { TITLE_BAR } else { placement.height.max(TITLE_BAR) };
    clamp_into(pos, egui::vec2(width, tall), bounds)
}

/// Keep a window's title bar inside `bounds`, so it can always be grabbed
/// again. Only the bar is kept in: a tall popup near the bottom edge would
/// otherwise be shoved upward every frame, which fights the drag.
fn clamp_into(pos: egui::Pos2, bar: egui::Vec2, bounds: egui::Rect) -> egui::Pos2 {
    let max_x = (bounds.right() - bar.x).max(bounds.left());
    let max_y = (bounds.bottom() - bar.y).max(bounds.top());
    egui::pos2(pos.x.clamp(bounds.left(), max_x), pos.y.clamp(bounds.top(), max_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(200.0, 100.0), egui::vec2(800.0, 600.0))
    }

    #[test]
    fn a_popup_opens_inside_the_viewport_it_belongs_to() {
        // Not in the middle of the screen the way a dialog does: it is drawn in
        // the viewport, and the viewport is not the window.
        let placed = clamp_into(bounds().left_top() + egui::vec2(16.0, 16.0), egui::vec2(320.0, TITLE_BAR), bounds());
        assert!(bounds().contains(placed), "{placed:?} is outside {:?}", bounds());
    }

    #[test]
    fn a_popup_dragged_off_the_edge_keeps_its_title_bar_in_reach() {
        // Every edge, because a window pushed out of any of them is a window
        // that cannot be dragged back -- there is nothing left to grab.
        let bar = egui::vec2(320.0, TITLE_BAR);
        for away in
            [egui::vec2(-4000.0, 0.0), egui::vec2(4000.0, 0.0), egui::vec2(0.0, -4000.0), egui::vec2(0.0, 4000.0)]
        {
            let out = clamp_into(bounds().center() + away, bar, bounds());
            assert!(bounds().contains(out), "dragged by {away:?} the bar landed at {out:?}");
            assert!(out.x + bar.x <= bounds().right() + 0.001, "the bar hangs off the right edge at {out:?}");
            assert!(out.y + bar.y <= bounds().bottom() + 0.001, "the bar hangs off the bottom edge at {out:?}");
        }
    }

    #[test]
    fn rolling_a_window_up_leaves_its_title_bar_exactly_where_it_was() {
        // The bug: a tall window near the bottom edge was lifted to fit, and
        // rolling it up removed the reason for the lift -- so the bar dropped
        // back down, out from under the chevron that had just been clicked.
        // Against a 600 px viewport, a 500 px window dropped at y = 400 was
        // lifted to y = 100, and collapsing it put it back at 400.
        let mut placement = Placement { pos: None, collapsed: false, height: 500.0 };
        placement.pos = Some(egui::pos2(300.0, 400.0));
        let open = settle(&placement, 320.0, bounds());
        assert!(open.y < 400.0, "a window taller than the room below it was not lifted to fit");
        placement.pos = Some(open);

        placement.collapsed = true;
        let rolled = settle(&placement, 320.0, bounds());
        assert_eq!(rolled, open, "rolling the window up moved its title bar");

        // And unrolling it does not move it either, because it was already
        // standing somewhere the whole window fits.
        placement.collapsed = false;
        assert_eq!(settle(&placement, 320.0, bounds()), open, "unrolling the window moved its title bar");
    }

    #[test]
    fn a_window_that_fits_where_it_was_left_is_not_moved_at_all() {
        let placement = Placement { pos: Some(egui::pos2(300.0, 200.0)), collapsed: false, height: 180.0 };
        assert_eq!(settle(&placement, 320.0, bounds()), egui::pos2(300.0, 200.0));
    }

    #[test]
    fn a_viewport_smaller_than_the_popup_still_leaves_it_somewhere_to_be() {
        // Dragging the panels out until the viewport is narrower than the
        // window must not produce a negative range to clamp into.
        let tiny = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(40.0, 20.0));
        let out = clamp_into(egui::pos2(500.0, 500.0), egui::vec2(320.0, TITLE_BAR), tiny);
        assert_eq!(out, tiny.left_top());
    }
}
