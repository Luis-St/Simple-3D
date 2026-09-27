//! The tool's window and its buttons.

use super::*;
use crate::app::App;
use crate::popup::{PopupEvent, PopupSpec};
use crate::ui;

/// The tool's window over the viewport, while open (issue 108).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    app.refresh_reassemble_tool();
    if app.reassemble_tool.is_none() {
        return;
    }
    // Named, since the non-modal window can outlive the selection.
    let title = app
        .reassemble_tool
        .as_ref()
        .and_then(|tool| app.scene.get(tool.target))
        .map_or_else(|| "Reassemble the mesh".to_string(), |node| format!("Reassemble {}", node.name));
    let spec = PopupSpec { key: KEY, title: &title, width: WIDTH };
    let event = app.tool_popup(ctx, spec, body, actions);
    if event == PopupEvent::Closed {
        app.cancel_reassemble_tool();
    }
}

/// The tool's contents: search settings, output settings, and what was found. The viewport draws
/// the findings ([`crate::reassemble_tool`]). The tool is lifted out of the app while drawing, since
/// the fields also borrow app state.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut tool) = app.reassemble_tool.take() else {
        ui.label("The mesh this was opened on is no longer there.");
        return;
    };
    controls(app, ui, &mut tool);
    ui.add_space(6.0);
    summary(ui, &tool);
    app.reassemble_tool = Some(tool);
}

pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui) {
    // Ready only once a run has landed and found something.
    let ready = app.reassemble_ready();
    if ui::dialog_button(ui, "Reassemble", ready).clicked() {
        app.apply_reassemble();
    }
    crate::app_chrome::cancel_at_left(ui, |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            app.cancel_reassemble_tool();
        }
    });
}
