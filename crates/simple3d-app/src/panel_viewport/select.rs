//! Picking what is under the pointer.

use crate::app::{App, Status};
use crate::pick;
use crate::view::View;
use simple3d_core::keymap::Command;

/// The measure tool's pointer handling: a click drops a point, snapped to the
/// nearest feature within reach, and Escape clears the span or puts the tool
/// away (issue 69).
pub(crate) fn measure_interact(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, view: &View) {
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        if app.measure.points.is_empty() {
            app.run(Command::MeasureTool);
        } else {
            app.measure.clear();
            app.status = Status::Info("Measurement cleared".into());
        }
        return;
    }
    // A right-click takes the last placed end back off, so a point put down in
    // the wrong place is undone where it was made rather than by reaching for
    // the panel. A right *drag* still orbits: only a click that never became one
    // is this gesture.
    if response.clicked_by(egui::PointerButton::Secondary) {
        app.measure_unplace();
        return;
    }
    if response.clicked_by(egui::PointerButton::Primary) {
        if let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) {
            if let Some(point) = app.measure_point_at(view, cursor) {
                app.measure_click(point);
            }
        }
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
}

pub(crate) fn select_under_cursor(app: &mut App, ui: &mut egui::Ui, view: &View) {
    let Some(cursor) = ui.input(|i| i.pointer.interact_pos()) else { return };
    let (origin, direction) = view.ray(cursor);
    let adding = ui.input(|i| i.modifiers.command || i.modifiers.shift);
    match pick::pick(&app.scene, &app.evaluated, origin, direction) {
        Some(hit) => {
            // A click on a piece still held inside a collection means the
            // collection, the way a click anywhere on a pattern's output means
            // the pattern: the piece has no row, and selecting something the
            // tree cannot show is selecting it out of sight (issue 82).
            let id = app.scene.row_for(hit);
            // Unless the collection is already what is selected. Then the click
            // is about the piece, and it ticks it in the panel's list -- which
            // is how a piece out of thousands is found at all: by pointing at
            // it, rather than by reading names off a list.
            if id != hit && app.listed_collection() == Some(id) {
                app.tick_piece(hit, adding);
                return;
            }
            if adding {
                app.toggle_selected(id);
            } else {
                app.select_only(id);
            }
        }
        None => {
            if !adding {
                app.clear_selection();
            }
        }
    }
}
