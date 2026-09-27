//! The tool's window and its buttons.

use super::*;
use crate::app::App;
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::ui;
use simple3d_geom::tiling::CellKind;

/// The tool's window over the viewport, while open (issue 82).
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    app.refresh_split_tool();
    if app.split_tool.is_none() {
        return;
    }
    let bounds = app.viewport_rect;
    // Named, since the non-modal window can outlive the selection.
    let title = app
        .split_tool
        .as_ref()
        .and_then(|tool| app.scene.get(tool.target))
        .map_or_else(|| "Split into smaller pieces".to_string(), |node| format!("Split {} into pieces", node.name));
    // Taken out of the map so the popup can hold it mutably while the contents hold the app.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event = popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: &title, width: WIDTH }, |ui| {
        // Scrolls on short viewports so Split and Cancel stay reachable.
        popup::scrolling_body(ui, bounds, |ui| body(app, ui));
        popup::action_row(ui, |ui| actions(app, ui));
    });
    app.popups.insert(KEY, placement);
    if event == PopupEvent::Closed {
        app.cancel_split_tool();
    }
}

/// What to call a number of pieces of a cell shape.
pub(crate) fn plural_cells(kind: CellKind, count: usize) -> String {
    if count == 1 {
        kind.singular().to_string()
    } else {
        kind.label().to_lowercase()
    }
}

/// The tool's contents: the cuts and what they come to. No picture: the viewport is the preview
/// ([`preview_loops`]). The tool is lifted out of the app while drawing, since the fields also
/// borrow app state.
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
    // Cancel sits at the far end from Split, which is laid out from the right.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if ui::dialog_button(ui, "Cancel", true).clicked() {
            app.cancel_split_tool();
        }
    });
}
