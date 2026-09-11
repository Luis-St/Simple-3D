//! The scatter's own window (issue 79).
//!
//! How far each copy may wander off where the rule puts it is six numbers, and
//! they used to unroll inside the properties panel under the pattern's own. That
//! is where they read worst: a scatter is judged entirely by what it does to the
//! model, and the six rows appeared by pushing the model's other numbers down
//! the panel and the panel's scroll position out from under the pointer. The
//! numbers now float over the viewport in an [in-place popup](crate::popup),
//! beside the pattern tool's -- dragged out of the way, rolled up to its bar,
//! and non-modal, so the copies can be orbited while the jitter is scrubbed.
//!
//! The window is also where the scatter is taken off again. Four numbers typed
//! back to zero by hand is the only way a pattern used to stop being scattered,
//! and three of them left at a tenth of a millimetre is a pattern that still is.

use crate::app::App;
use crate::panel_properties::{param_field, PATTERN_ROW};
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::theme;
use crate::ui;
use simple3d_core::pattern;
use simple3d_core::scene::NodeId;

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
        let (area, restore) = theme::list_scroll_area(ui);
        area.auto_shrink([false, true]).max_height(popup::body_room(bounds)).show(ui, |ui| {
            ui.set_style(restore);
            body(app, ui, id);
        });
        popup::action_row(ui, |ui| actions(app, ui, id));
    });
    app.popups.insert(KEY, placement);
    if event == PopupEvent::Closed {
        app.noise_popup = None;
    }
}

/// The six numbers the scatter is made of, and what they currently come to.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let unit = app.unit();
    let targets = [id];
    for key in pattern::noise_keys() {
        let Some(spec) = pattern::PARAMS.iter().find(|param| param.key == *key) else { continue };
        param_field(app, ui, &targets, id, spec, unit, PATTERN_ROW);
    }
    let Some(params) = app.scene.node(id).params() else { return };
    let scatter = pattern::Noise::of(params);
    // Said rather than left to be read off four fields: the three distances are
    // a box a copy may land anywhere in, which is not what three separate
    // numbers look like, and a scatter asked for in millimetres on a pattern
    // spaced in metres is one nobody can see.
    let note = if scatter.wanted() {
        "Every copy lands somewhere inside this, and stays there: the same seed scatters the same way every time \
         the file is opened."
    } else {
        "Nothing yet -- the copies are exactly where the rule puts them. Give a distance or a turn and they \
         wander off it."
    };
    ui.add(egui::Label::new(theme::hint(note)).selectable(false));
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

impl App {
    /// Whether anything here has been moved off what a fresh pattern holds.
    ///
    /// Asked of all six rather than of the scatter itself: a seed stepped past a
    /// scatter that looked wrong, and an axis turned from Z, are both things the
    /// user set and both things Reset puts back, so a button that went grey
    /// while they still said something would be lying about what it does.
    pub(crate) fn noise_is_set(&self, id: NodeId) -> bool {
        let Some(params) = self.scene.get(id).and_then(|node| node.params()) else { return false };
        pattern::noise_keys().iter().any(|key| match default_of(key) {
            Some(default) => params.get(*key).is_some_and(|value| *value != default),
            None => false,
        })
    }

    /// Take the scatter off: every one of the six back to what a fresh pattern
    /// holds.
    ///
    /// One undo step for the lot. Four numbers typed back to zero by hand is
    /// four of them, and stopping halfway through leaves a pattern that is still
    /// scattered by whatever is left.
    pub(crate) fn reset_noise(&mut self, id: NodeId) {
        if !self.noise_is_set(id) {
            return;
        }
        self.edit("Reset noise", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|node| node.params_mut()) {
            for key in pattern::noise_keys() {
                if let Some(default) = default_of(key) {
                    params.insert(key.to_string(), default);
                }
            }
        }
        self.touch();
    }
}

/// What a fresh pattern holds for one of the scatter's parameters.
fn default_of(key: &str) -> Option<simple3d_core::primitive::ParamValue> {
    pattern::PARAMS.iter().find(|param| param.key == key).map(|param| param.default)
}
