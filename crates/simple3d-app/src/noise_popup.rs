//! The scatter's own window (issue 79).
//!
//! How far each copy may wander off where the rule puts it is a handful of numbers, and
//! they used to unroll inside the properties panel under the pattern's own. That
//! is where they read worst: a scatter is judged entirely by what it does to the
//! model, and the rows appeared by pushing the model's other numbers down
//! the panel and the panel's scroll position out from under the pointer. The
//! numbers now float over the viewport in an [in-place popup](crate::popup),
//! beside the pattern tool's -- dragged out of the way, rolled up to its bar,
//! and non-modal, so the copies can be orbited while the jitter is scrubbed.
//!
//! The window is also where the scatter is taken off again. Four numbers typed
//! back to zero by hand is the only way a pattern used to stop being scattered,
//! and three of them left at a tenth of a millimetre is a pattern that still is.
//!
//! What is in it is built rather than filled in (issue 79). A scatter is three
//! parts -- a nudge, a turn, a change of size -- and it used to be eight fields
//! at once, all at nothing, whichever of them the pattern actually used. Now
//! each part is added from a chip, as a card with only its own numbers and a
//! cross that takes that part off, and the seed that picks one scatter out of
//! all the scatters of that size has a button that steps to the next. The same
//! builder is the last card of the pattern tool, for a rule being built there.

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

/// Identifies the popup, and is what remembers where it was dragged to.
const KEY: &str = "pattern-noise";

/// How wide the window is: a jitter's name and its field side by side, and no
/// wider. A popup lives over the model, so every pixel of it is a pixel of the
/// scatter being laid out that cannot be seen.
const WIDTH: f32 = 300.0;

/// The window, drawn over the viewport once a frame while it is open.
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    // The pattern can go while the window is up -- this is not modal, and the
    // outliner behind it can delete what it is working on.
    if app.noise_popup.is_some_and(|id| !app.scene.get(id).is_some_and(|node| node.is_pattern())) {
        app.noise_popup = None;
    }
    let Some(id) = app.noise_popup else { return };
    let bounds = app.viewport_rect;
    // The window names what it is scattering. It has to: the selection can move
    // on to something else while it is open.
    let title = format!("Noise for {}", app.scene.node(id).name);
    // Taken out of the map for the duration, so the popup may hold it mutably
    // while its contents hold the application.
    let mut placement = app.popups.remove(KEY).unwrap_or_default();
    let event = popup::show(ctx, bounds, &mut placement, PopupSpec { key: KEY, title: &title, width: WIDTH }, |ui| {
        // Six rows and a hint is a short window until the rows stack on a narrow
        // one, and a viewport can be short: the body scrolls rather than pushing
        // the buttons off the bottom of the screen where nothing can reach them.
        popup::scrolling_body(ui, bounds, |ui| builder(app, ui, id, PATTERN_ROW));
        popup::action_row(ui, |ui| actions(app, ui, id));
    });
    app.popups.insert(KEY, placement);
    if event == PopupEvent::Closed {
        app.noise_popup = None;
    }
}

/// The window's buttons: take the scatter off, or put the window away.
///
/// Done rather than Close: nothing here is provisional. Every number typed is
/// already on the pattern, so shutting the window ends the job rather than
/// abandoning it -- there is no cancel to offer.
pub(crate) fn actions(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    // The action row lays itself out from the right, so these are added in the
    // order they are read backwards.
    if ui::dialog_button(ui, "Done", true).clicked() {
        app.noise_popup = None;
    }
    // Reset is at the other end of the row, the way the measure tool's Clear is:
    // the button that throws away what has been set is as far as the window is
    // wide from the one that is pressed to finish, so the pointer on its way to
    // Done never passes over it.
    //
    // Greyed out when there is nothing to undo, so the button says whether this
    // pattern is scattered at all as well as offering to stop it.
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        if ui::dialog_button(ui, "Reset", app.noise_is_set(id))
            .on_hover_text("Put every one of these back to none, so the copies land exactly where the rule puts them")
            .clicked()
        {
            app.reset_noise(id);
        }
    });
}
