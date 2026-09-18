//! The tool's window and the buttons on it.

use super::*;
use crate::app::App;
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::ui;

/// The tool's own window, drawn over the viewport once a frame while it is open
/// (issue 106).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    app.refresh_simplify_tool();
    if app.simplify_tool.is_none() {
        return;
    }
    let bounds = app.viewport_rect;
    // The window names what it is simplifying. It has to: it is not modal, so
    // the selection can move on to something else while it is open, and a
    // window that only said "Simplify" would leave no way to tell which mesh is
    // about to lose its triangles.
    let title = app
        .simplify_tool
        .as_ref()
        .and_then(|tool| app.scene.get(tool.target))
        .map_or_else(|| "Simplify the mesh".to_string(), |node| format!("Simplify {}", node.name));
    // Taken out of the map for the duration, so the popup may hold it mutably
    // while its contents hold the application.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event = popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: &title, width: WIDTH }, |ui| {
        // Six rows and a summary is a short window until the rows stack on a
        // narrow one, and a viewport can be short: the body scrolls rather than
        // pushing the buttons off the bottom of the screen where nothing can
        // reach them.
        popup::scrolling_body(ui, bounds, |ui| body(app, ui));
        popup::action_row(ui, |ui| actions(app, ui));
    });
    app.popups.insert(KEY, placement);
    if event == PopupEvent::Closed {
        app.cancel_simplify_tool();
    }
}

/// The tool's contents: what to drop, what to keep, and what that came to.
///
/// No picture in the window. The picture is the viewport, and it is not a
/// picture -- it is the result, standing in the document while the window is
/// open. See [the module's own documentation](crate::simplify_tool) for why
/// that is the way round it is.
///
/// The tool is lifted out of the application for the length of the drawing and
/// put back at the end: the fields are the properties panel's own control, and
/// that control lives on the application, so the two cannot be borrowed from it
/// at once.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut tool) = app.simplify_tool.take() else {
        ui.label("The mesh this was opened on is no longer there.");
        return;
    };
    controls(app, ui, &mut tool);
    ui.add_space(6.0);
    summary(app, ui, &tool);
    app.simplify_tool = Some(tool);
}

pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    // Ready only once a run has landed, because Simplify keeps *what is on
    // screen*: pressing it while the first run is still going would either wait
    // with the interface stopped or keep a mesh nobody has seen.
    let ready = app.simplify_tool.as_ref().is_some_and(|tool| tool.shown.is_some() && app.scene.contains(tool.target));
    if ui::dialog_button(ui, "Simplify", ready).clicked() {
        app.apply_simplify();
    }
    // Cancel is at the other end of the row, not beside Simplify. The row is
    // laid out from the right, so the button that goes through with the command
    // sits under the pointer's own corner; the one that throws the window away
    // -- and the mesh with it -- is as far from it as the window is wide.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            app.cancel_simplify_tool();
        }
    });
}
