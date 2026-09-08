//! The tool's window and the buttons on it.

use super::*;
use crate::app::App;
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::{theme, ui};
use simple3d_geom::tiling::CellKind;

/// The tool's own window, drawn over the viewport once a frame while it is open
/// (issue 82).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    app.refresh_split_tool();
    if app.split_tool.is_none() {
        return;
    }
    let bounds = app.viewport_rect;
    // The window names what it is cutting. It has to: it is not modal, so the
    // selection can move on to something else while it is open, and a window
    // that only says "Split into smaller pieces" would leave no way to tell
    // which object is about to be cut.
    let title = app
        .split_tool
        .as_ref()
        .and_then(|tool| app.scene.get(tool.target))
        .map_or_else(|| "Split into smaller pieces".to_string(), |node| format!("Split {} into pieces", node.name));
    // Taken out of the map for the duration, so the popup may hold it mutably
    // while its contents hold the application.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event = popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: &title, width: WIDTH }, |ui| {
        // Three cuts is three columns of fields, which is taller than a short
        // viewport: the body scrolls rather than pushing Split and Cancel off
        // the bottom of the screen where nothing can reach them.
        let (area, restore) = theme::list_scroll_area(ui);
        area.auto_shrink([false, true]).max_height(popup::body_room(bounds)).show(ui, |ui| {
            ui.set_style(restore);
            body(app, ui);
        });
        popup::action_row(ui, |ui| actions(app, ui));
    });
    app.popups.insert(KEY, placement);
    if event == PopupEvent::Closed {
        app.cancel_split_tool();
    }
}

/// What to call a number of pieces of a given cell shape.
pub(crate) fn plural_cells(kind: CellKind, count: usize) -> String {
    if count == 1 {
        kind.singular().to_string()
    } else {
        kind.label().to_lowercase()
    }
}

/// The tool's contents: the cuts to make, and what they come to.
///
/// One column of fields and no picture. The picture is the viewport -- see
/// [`preview_loops`] -- which is the whole reason the window is a popup
/// floating over it rather than a dialog in front of it: a plan drawn small
/// inside the window answers "what shape are the cells", and the model behind
/// it answers "where will they fall", which is the question actually being
/// asked.
///
/// The tool is lifted out of the application for the length of the drawing and
/// put back at the end. The fields are the same control the properties panel's
/// rows are -- dragged to change the number, clicked to type it -- and that
/// control lives on the application, so the two cannot be borrowed from it at
/// once.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut tool) = app.split_tool.take() else {
        ui.label("The object this was opened on is no longer there.");
        return;
    };
    controls(app, ui, &mut tool);
    ui.add_space(6.0);
    summary(app, ui, &tool);
    app.split_tool = Some(tool);
}

pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    let ready = app
        .split_tool
        .as_ref()
        .is_some_and(|tool| tool.plan.refusal(tool.bounds).is_none() && app.scene.contains(tool.target));
    if ui::dialog_button(ui, "Split", ready).clicked() {
        app.start_split();
    }
    // Cancel is at the other end of the row, not beside Split. The row is laid
    // out from the right, so the button that goes through with the command sits
    // under the pointer's own corner; the one that throws the window away is as
    // far from it as the window is wide, which is the distance a press nobody
    // meant has to cross.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            app.cancel_split_tool();
        }
    });
}
