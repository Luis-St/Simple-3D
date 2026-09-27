//! The tool's window and its buttons.

use super::*;
use crate::app::App;
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::ui;

/// The tool's window over the viewport, while open (issue 106).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    app.refresh_simplify_tool();
    if app.simplify_tool.is_none() {
        return;
    }
    let bounds = app.viewport_rect;
    // Named, since the non-modal window can outlive the selection.
    let title = app
        .simplify_tool
        .as_ref()
        .and_then(|tool| app.scene.get(tool.target))
        .map_or_else(|| "Simplify the mesh".to_string(), |node| format!("Simplify {}", node.name));
    // Taken out of the map so the popup can hold it mutably while the contents hold the app.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event = popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: &title, width: WIDTH }, |ui| {
        // Scrolls on short viewports so the buttons stay reachable.
        popup::scrolling_body(ui, bounds, |ui| body(app, ui));
        popup::action_row(ui, |ui| actions(app, ui));
    });
    app.popups.insert(KEY, placement);
    if event == PopupEvent::Closed {
        app.cancel_simplify_tool();
    }
}

/// The tool's contents: what to drop, what to keep, and the result. The viewport shows the real
/// result ([`crate::simplify_tool`]). The tool is lifted out of the app while drawing, since the
/// fields also borrow app state.
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
    // Ready only once a run has landed, since Simplify keeps what is on screen.
    let ready = app.simplify_tool.as_ref().is_some_and(|tool| tool.shown.is_some() && app.scene.contains(tool.target));
    if ui::dialog_button(ui, "Simplify", ready).clicked() {
        app.apply_simplify();
    }
    // Cancel sits at the far end from Simplify, which is laid out from the right.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            app.cancel_simplify_tool();
        }
    });
}
