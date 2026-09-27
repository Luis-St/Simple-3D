//! The scatter's own window (issue 79).
//!
//! A non-modal [in-place popup](crate::popup) beside the pattern tool, so copies can be orbited
//! while scrubbing and the properties panel does not reflow. It can also remove the scatter
//! outright. Built from parts (nudge, turn, size) added as cards from chips, with a button stepping
//! the seed; the same builder is the pattern tool's last card.

use crate::app::App;
use crate::panel_properties::PATTERN_ROW;
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::ui;
use simple3d_core::scene::NodeId;

mod part;
pub(crate) use part::*;
mod builder;
pub(crate) use builder::*;
mod edits;

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "pattern-noise";

/// The window width: a jitter's name and field side by side, no wider.
const WIDTH: f32 = 300.0;

/// The window over the viewport, while open.
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    // Not modal, so the pattern may be deleted while it is open.
    if app.noise_popup.is_some_and(|id| !app.scene.get(id).is_some_and(|node| node.is_pattern())) {
        app.noise_popup = None;
    }
    let Some(id) = app.noise_popup else { return };
    let bounds = app.viewport_rect;
    // Named, since the selection can move on while it is open.
    let title = format!("Noise for {}", app.scene.node(id).name);
    // Taken out of the map so the popup can hold it mutably while the contents hold the app.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event = popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: &title, width: WIDTH }, |ui| {
        // Scrolls on short viewports so the buttons stay reachable.
        popup::scrolling_body(ui, bounds, |ui| builder(app, ui, id, PATTERN_ROW));
        popup::action_row(ui, |ui| actions(app, ui, id));
    });
    app.popups.insert(KEY, placement);
    if event == PopupEvent::Closed {
        app.noise_popup = None;
    }
}

/// The buttons: remove the scatter, or Done (nothing is provisional, so there is no cancel).
pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    // The action row lays out from the right, so buttons are added in reverse reading order.
    if ui::dialog_button(ui, "Done", true).clicked() {
        app.noise_popup = None;
    }
    // Reset sits at the far end from Done, and is greyed out when there is no scatter.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if ui::dialog_button(ui, "Reset", app.noise_is_set(id))
            .on_hover_text("Put every one of these back to none, so the copies land exactly where the rule puts them")
            .clicked()
        {
            app.reset_noise(id);
        }
    });
}
