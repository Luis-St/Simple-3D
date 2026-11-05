//! The tool's pointer: click the profile, then the line to turn about; the popup sets the turn.

use super::*;
use crate::app::{App, Status};
use crate::view::View;

/// The tool's pointer for one frame. Returns whether it took the pointer, so a click does not also
/// select what is under it.
pub(crate) fn interact(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) -> bool {
    let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if (escape || response.clicked_by(egui::PointerButton::Secondary)) && app.revolve.back() {
        app.status = Status::Info(step_hint(&app.revolve).into());
        return true;
    }
    let cursor = ui.input(|i| i.pointer.hover_pos());
    let click =
        response.clicked_by(egui::PointerButton::Primary).then(|| ui.input(|i| i.pointer.interact_pos())).flatten();
    if response.hovered() {
        if let Some(cursor) = cursor {
            match under(app, view, cursor) {
                Some(Under::Axis(_)) => ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair),
                Some(Under::Profile(_)) => ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand),
                _ => {}
            }
        }
    }
    let Some(at) = click else { return false };
    match under(app, view, at) {
        Some(Under::Axis(axis)) => app.revolve.set_axis(axis),
        Some(Under::Profile(target)) => app.revolve.set_profile(target),
        Some(Under::Refused(why)) => {
            app.status = Status::Warning(why.into());
            return true;
        }
        None => return false,
    }
    app.status = Status::Info(step_hint(&app.revolve).into());
    true
}

/// What a click at a point would take.
pub(crate) enum Under {
    Axis(AxisPick),
    Profile(Target),
    Refused(&'static str),
}

/// What a click at `at` takes: with a profile picked, a line under it as the axis first, else a
/// region or face as the profile instead.
pub(crate) fn under(app: &App, view: &View, at: egui::Pos2) -> Option<Under> {
    let mut refused = None;
    if let Some(profile) = &app.revolve.profile {
        match app.revolve_axis_under(view, at, profile) {
            Some(Ok(axis)) => return Some(Under::Axis(axis)),
            Some(Err(why)) => refused = Some(why),
            None => {}
        }
    }
    match app.push_target_under(view, at) {
        Some((Ok(target), _)) => Some(Under::Profile(target)),
        Some((Err(why), _)) => Some(Under::Refused(refused.unwrap_or(why))),
        None => refused.map(Under::Refused),
    }
}

/// What to do next, for the status line and the popup.
pub(crate) fn step_hint(tool: &Revolve) -> &'static str {
    if tool.profile.is_none() {
        "Revolve: click a closed region of a sketch or a flat face to turn"
    } else if tool.axis.is_none() {
        "Revolve: click a line of the sketch or a straight edge to turn about, or choose an axis in the popup"
    } else {
        "Revolve: set the angle in the popup and apply; click another line or profile to change it"
    }
}
