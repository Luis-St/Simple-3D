//! The tool's window and its buttons.

use super::*;
use crate::app::App;
use crate::popup::{PopupEvent, PopupSpec};
use crate::ui;

/// The tool's window over the viewport, while open (issue 81).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    app.refresh_shape_tool();
    if app.shape_tool.is_none() {
        return;
    }
    // Named, since the non-modal window can outlive the selection.
    let title = app
        .shape_tool
        .as_ref()
        .and_then(|tool| app.scene.get(tool.target))
        .map_or_else(|| "Apply a function".to_string(), |node| format!("Apply a function to {}", node.name));
    let spec = PopupSpec { key: KEY, title: &title, width: WIDTH };
    let event = app.tool_popup(ctx, spec, body, actions);
    if event == PopupEvent::Closed {
        app.cancel_shape_tool();
    }
}

/// The tool's contents: the function, its numbers, and what it came to. The tool is lifted out of the
/// app while drawing, since the fields also borrow app state.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut tool) = app.shape_tool.take() else {
        ui.label("The mesh this was opened on is no longer there.");
        return;
    };
    controls(app, ui, &mut tool);
    ui.add_space(6.0);
    summary(app, ui, &tool);
    app.shape_tool = Some(tool);
}

pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    // Ready only once a run has landed, since Apply keeps what is on screen.
    let ready = app.shape_tool.as_ref().is_some_and(|tool| tool.shown.is_some() && app.scene.contains(tool.target));
    if ui::dialog_button(ui, "Apply", ready).clicked() {
        app.apply_shape();
    }
    crate::app_chrome::cancel_at_left(ui, |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            app.cancel_shape_tool();
        }
    });
}
