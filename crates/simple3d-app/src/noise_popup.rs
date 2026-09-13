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
use crate::panel_properties::{
    field_row, field_row_boxed, number_field, param_field_as, room_left, vector_row, RowStyle, PATTERN_ROW,
};
use crate::pattern_tool::{add_chip, length, note};
use crate::popup::{self, PopupEvent, PopupSpec};
use crate::theme::{self, token};
use crate::ui;
use simple3d_core::pattern;
use simple3d_core::primitive::{ParamValue, Params, ParamsExt};
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

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

/// The parts a scatter is built from, in the order the builder lists them
/// (issue 79).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    /// Moved off where the rule put it, along each axis.
    Nudge,
    /// Turned where it stands, about one axis. A scatter takes one for each.
    Turn(usize),
    /// Made bigger or smaller.
    Size,
}

/// The axes a turn is added about, in the order "+ Turn" takes them: Z first,
/// the way a plank lies askew on a floor.
const TURN_ORDER: [usize; 3] = [2, 0, 1];

/// The axes' names, by index.
const AXES: [&str; 3] = ["X", "Y", "Z"];

impl Part {
    pub(crate) const ALL: [Part; 5] = [Part::Nudge, Part::Turn(0), Part::Turn(1), Part::Turn(2), Part::Size];

    /// How many parts there are, for what the builder keeps open.
    pub(crate) const COUNT: usize = Part::ALL.len();

    fn index(self) -> usize {
        match self {
            Part::Nudge => 0,
            Part::Turn(axis) => 1 + axis.min(2),
            Part::Size => 4,
        }
    }

    fn name(self) -> String {
        match self {
            Part::Nudge => "Nudge".to_string(),
            Part::Turn(axis) => format!("Turn {}", AXES[axis.min(2)]),
            Part::Size => "Size".to_string(),
        }
    }

    fn hover(self) -> &'static str {
        match self {
            Part::Nudge => "Move each copy a little off where the rule puts it",
            Part::Turn(_) => "Turn each copy a little askew where it stands, about one axis. Add one for each axis.",
            Part::Size => "Make each copy a little bigger or smaller",
        }
    }

    /// The numbers it is made of, which are what taking it off puts back to
    /// nothing.
    fn keys(self) -> &'static [&'static str] {
        match self {
            Part::Nudge => &["noise_x", "noise_y", "noise_z"],
            Part::Turn(0) => &["noise_turn_x"],
            Part::Turn(1) => &["noise_turn_y"],
            Part::Turn(_) => &["noise_turn_z"],
            Part::Size => &["noise_scale"],
        }
    }

    fn in_use(self, params: &Params) -> bool {
        self.keys().iter().any(|key| params.num(key).abs() > 1e-9)
    }
}

/// The id of the chip that adds `part`, in the builder drawn under `scope`:
/// the tool and the window can both be up on one pattern.
pub(crate) fn part_id(scope: &str, part: Part) -> egui::Id {
    egui::Id::new(("noise-add-part", scope.to_string(), part.index()))
}

/// The id of the chip that adds a turn about the next axis without one.
pub(crate) fn add_turn_id(scope: &str) -> egui::Id {
    egui::Id::new(("noise-add-turn", scope.to_string()))
}

/// The id of the chip that moves a turn onto `axis`.
pub(crate) fn turn_axis_id(scope: &str, from: usize, to: usize) -> egui::Id {
    egui::Id::new(("noise-turn-axis", scope.to_string(), from, to))
}

/// The id of the cross that takes `part` off.
pub(crate) fn drop_part_id(scope: &str, part: Part) -> egui::Id {
    egui::Id::new(("noise-drop-part", scope.to_string(), part.index()))
}

/// The id of the button that steps the seed on.
pub(crate) fn shuffle_id(scope: &str) -> egui::Id {
    egui::Id::new(("noise-shuffle", scope.to_string()))
}

/// What one of the builder's controls asked for. Acted on after it is drawn.
#[derive(Clone, Copy)]
enum Ask {
    Add(Part),
    Drop(Part),
    /// Move the turn about the first axis onto the second.
    Retarget(usize, usize),
    Shuffle,
}

/// The scatter, built part by part: a card for each part in use, the chips
/// that add the rest, and -- once there is anything to scatter -- the seed,
/// whether the original stays put, and whether the copies can meet.
pub(crate) fn builder(app: &mut App, ui: &mut egui::Ui, id: NodeId, style: RowStyle) {
    let Some(params) = app.scene.node(id).params().cloned() else { return };
    let unit = app.unit();
    let scope = style.scope();
    let shown: Vec<Part> =
        Part::ALL.into_iter().filter(|part| part.in_use(&params) || app.noise_part_open(id, *part)).collect();
    let mut ask = None;
    for part in &shown {
        ui.add_space(4.0);
        crate::pattern_tool::card(token::SURFACE_1).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if crate::pattern_tool::card_heading(
                ui,
                &part.name(),
                &summary(*part, &params, unit),
                drop_part_id(scope, *part),
                "Take this part of the noise off",
            ) {
                ask = Some(Ask::Drop(*part));
            }
            match part {
                Part::Nudge => vector_row(
                    app,
                    ui,
                    id,
                    ["noise_x", "noise_y", "noise_z"],
                    "Up to",
                    "The furthest a copy may be moved along each axis, either way",
                    style,
                ),
                Part::Turn(axis) => {
                    field(app, ui, id, part.keys()[0], "Up to", style);
                    turn_axis_row(ui, scope, *axis, &shown, &mut ask);
                }
                Part::Size => field(app, ui, id, "noise_scale", "Up to \u{00B1} (%)", style),
            }
        });
    }
    // What is left to add: the nudge and the size once each, and a turn while
    // an axis is still without one.
    let next_turn = TURN_ORDER.into_iter().map(Part::Turn).find(|turn| !shown.contains(turn));
    let addable: Vec<Part> = [Some(Part::Nudge), next_turn, Some(Part::Size)]
        .into_iter()
        .flatten()
        .filter(|part| !shown.contains(part))
        .collect();
    if !addable.is_empty() {
        ui.add_space(4.0);
        field_row_boxed(ui, "Add", "Let the copies wander off where the rule puts them", |ui| {
            for part in addable {
                // A turn's chip is one chip whichever axis it adds next.
                let (text, named) = match part {
                    Part::Turn(_) => ("+ Turn".to_string(), add_turn_id(scope)),
                    other => (format!("+ {}", other.name()), part_id(scope, other)),
                };
                if add_chip(ui, &text, part.hover(), named) {
                    ask = Some(Ask::Add(part));
                }
            }
        });
    }
    if shown.is_empty() {
        note(
            ui,
            "None: every copy is exactly where the rule puts it. Add a part and the copies wander off it by no more \
             than you say, like planks laid a little askew or stones along a path.",
        );
    } else {
        ui.add_space(4.0);
        seed_row(app, ui, id, style, &mut ask);
        field(app, ui, id, "noise_keep_first", "Keep the original", style);
        note(
            ui,
            "Every copy lands somewhere inside this, and stays there: the same seed scatters the same way every time \
             the file is opened.",
        );
        crowding_note(app, ui, id, &params);
    }
    match ask {
        Some(Ask::Add(part)) => app.add_noise_part(id, part),
        Some(Ask::Drop(part)) => app.drop_noise_part(id, part),
        Some(Ask::Retarget(from, to)) => app.move_noise_turn(id, from, to),
        Some(Ask::Shuffle) => app.shuffle_noise(id),
        None => {}
    }
}

/// The seed, and the button that steps it on: the same amounts, scattered
/// another way. Typing numbers into a seed to find a scatter that looks right
/// is clicking a button with extra steps.
fn seed_row(app: &mut App, ui: &mut egui::Ui, id: NodeId, style: RowStyle, ask: &mut Option<Ask>) {
    let Some(spec) = pattern::param_spec("noise_seed") else { return };
    let unit = app.unit();
    let scope = style.scope();
    field_row(
        ui,
        "Seed",
        "The same seed scatters the same way every time the file is opened. Another seed gives another \
         scatter of the same size.",
        |ui| {
            let width = (room_left(ui) - SHUFFLE - ui.spacing().item_spacing.x).max(40.0);
            ui.scope(|ui| {
                ui.set_width(width);
                number_field(app, ui, &[id], spec, unit, style, egui::Id::new(("noise-seed", id, scope)));
            });
            let shuffle = ui
                .add_sized(egui::vec2(SHUFFLE, theme::metric::INPUT_ROW), egui::Button::new("Shuffle"))
                .on_hover_text("The next seed: the same amounts, scattered another way");
            ui.interact(shuffle.rect, shuffle_id(scope), egui::Sense::hover());
            if shuffle.clicked() {
                *ask = Some(Ask::Shuffle);
            }
        },
    );
}

/// How wide the Shuffle button is.
const SHUFFLE: f32 = 64.0;

/// One of the scatter's rows, under the name the builder gives it.
fn field(app: &mut App, ui: &mut egui::Ui, id: NodeId, key: &str, name: &str, style: RowStyle) {
    let Some(spec) = pattern::param_spec(key) else { return };
    let unit = app.unit();
    param_field_as(app, ui, &[id], id, spec, name, unit, style);
}

/// What a turn is about, as three chips. The axes that already have a turn of
/// their own are greyed: a scatter holds one turn about each, and moving this
/// one onto another's would be two turns about one axis.
fn turn_axis_row(ui: &mut egui::Ui, scope: &str, axis: usize, shown: &[Part], ask: &mut Option<Ask>) {
    field_row(ui, "About", "", |ui| {
        for (other, name) in AXES.into_iter().enumerate() {
            let taken = other != axis && shown.contains(&Part::Turn(other));
            let chip = ui.add_enabled_ui(!taken, |ui| theme::choice(ui, other == axis, name)).inner;
            // Named, so a test can find it. It senses nothing; the chip answers.
            ui.interact(chip.rect, turn_axis_id(scope, axis, other), egui::Sense::hover());
            if chip.clicked() && other != axis {
                *ask = Some(Ask::Retarget(axis, other));
            }
        }
    });
}

/// What one part currently comes to, in a few words.
fn summary(part: Part, params: &Params, unit: simple3d_core::unit::Unit) -> String {
    use simple3d_core::unit::format_number;
    if !part.in_use(params) {
        return "nothing yet".to_string();
    }
    let scatter = pattern::Noise::of(params);
    match part {
        Part::Nudge => {
            let most = scatter.offset.x.max(scatter.offset.y).max(scatter.offset.z);
            format!("up to {} either way", length(most, unit))
        }
        Part::Turn(axis) => {
            let turn = [scatter.turn.x, scatter.turn.y, scatter.turn.z][axis.min(2)];
            format!("up to {} deg about {}", format_number(turn, 1), AXES[axis.min(2)])
        }
        Part::Size => format!("up to \u{00B1}{} %", format_number(scatter.scale * 100.0, 0)),
    }
}

/// Whether the scatter can make two copies meet, said beside the numbers that
/// did it.
///
/// Copies that meet are welded into one body -- a pattern unions them -- so a
/// scatter that can close the gaps the rule leaves turns a deck of planks into
/// a slab, and the only sign of it in the viewport is that the joints have
/// gone.
fn crowding_note(app: &App, ui: &mut egui::Ui, id: NodeId, params: &Params) {
    let unit = app.unit();
    let Some(crowded) = app.pattern_content_size(id).and_then(|size| pattern::crowding(params, size)) else {
        return;
    };
    ui.add(
        egui::Label::new(
            egui::RichText::new(format!(
                "Copies may meet: the rule leaves {} between neighbours, and two of them wandering towards each other \
                 can close {}. Copies that touch are welded into one body.",
                length(crowded.gap, unit),
                length(crowded.reach, unit)
            ))
            .size(theme::font::SMALL)
            .color(theme::token::DANGER),
        )
        .selectable(false)
        .wrap(),
    );
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
    /// Asked of all of them rather than of the scatter itself: a seed stepped
    /// past a scatter that looked wrong is something the user set and something
    /// Reset puts back, so a button that went grey while it still said something
    /// would be lying about what it does.
    pub(crate) fn noise_is_set(&self, id: NodeId) -> bool {
        let Some(params) = self.scene.get(id).and_then(|node| node.params()) else { return false };
        pattern::noise_keys()
            .iter()
            .any(|key| default_of(key).is_some_and(|default| params.get(*key).is_some_and(|value| *value != default)))
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
        self.reset_noise_keys(id, pattern::noise_keys());
        self.noise_parts_open = (None, [false; Part::COUNT]);
        self.touch();
    }

    /// Put `keys` back to what a fresh pattern holds, as part of an edit already
    /// begun.
    fn reset_noise_keys(&mut self, id: NodeId, keys: &[&str]) {
        let Some(params) = self.scene.get_mut(id).and_then(|node| node.params_mut()) else { return };
        for key in keys {
            if let Some(default) = default_of(key) {
                params.insert(key.to_string(), default);
            }
        }
    }

    /// Whether the builder is keeping `part` on screen for `id` although it is
    /// at nothing.
    fn noise_part_open(&self, id: NodeId, part: Part) -> bool {
        self.noise_parts_open.0 == Some(id) && self.noise_parts_open.1[part.index()]
    }

    fn set_noise_part_open(&mut self, id: NodeId, part: Part, open: bool) {
        if self.noise_parts_open.0 != Some(id) {
            self.noise_parts_open = (Some(id), [false; Part::COUNT]);
        }
        self.noise_parts_open.1[part.index()] = open;
    }

    /// Add one part to the scatter, at an amount that shows what it does.
    ///
    /// A nudge is a twentieth of the shape's narrower side, along the two axes
    /// a plank laid on a floor wanders in: enough to see, and well short of the
    /// gap a fresh pattern leaves between its copies, so adding it does not
    /// weld them together. A turn is three degrees, a size five percent.
    pub(crate) fn add_noise_part(&mut self, id: NodeId, part: Part) {
        let size = self.pattern_content_size(id).unwrap_or(Vec3::ZERO);
        self.edit("Add noise", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|node| node.params_mut()) {
            match part {
                Part::Nudge => {
                    let narrow = size.x.min(size.y);
                    let amount = if narrow > 1e-9 { narrow * 0.05 } else { 0.5 };
                    params.insert("noise_x".to_string(), ParamValue::Length(amount));
                    params.insert("noise_y".to_string(), ParamValue::Length(amount));
                }
                Part::Turn(axis) => {
                    params.insert(pattern::NOISE_TURN_KEYS[axis.min(2)].to_string(), ParamValue::Angle(3.0));
                }
                Part::Size => {
                    params.insert("noise_scale".to_string(), ParamValue::Count(5));
                }
            }
        }
        self.set_noise_part_open(id, part, true);
        self.touch();
    }

    /// Take one part of the scatter off, leaving the others as they are.
    pub(crate) fn drop_noise_part(&mut self, id: NodeId, part: Part) {
        self.edit("Remove noise", None);
        self.reset_noise_keys(id, part.keys());
        self.set_noise_part_open(id, part, false);
        self.touch();
    }

    /// Move the turn about `from` onto `to`, amount and all -- unless `to` has a
    /// turn of its own already.
    pub(crate) fn move_noise_turn(&mut self, id: NodeId, from: usize, to: usize) {
        let (from, to) = (from.min(2), to.min(2));
        if from == to || self.noise_part_open(id, Part::Turn(to)) {
            return;
        }
        let Some(params) = self.scene.get(id).and_then(|node| node.params()) else { return };
        if Part::Turn(to).in_use(params) {
            return;
        }
        let amount = params.num(pattern::NOISE_TURN_KEYS[from]);
        self.edit("Set noise", None);
        if let Some(params) = self.scene.get_mut(id).and_then(|node| node.params_mut()) {
            params.insert(pattern::NOISE_TURN_KEYS[from].to_string(), ParamValue::Angle(0.0));
            params.insert(pattern::NOISE_TURN_KEYS[to].to_string(), ParamValue::Angle(amount));
        }
        self.set_noise_part_open(id, Part::Turn(from), false);
        self.set_noise_part_open(id, Part::Turn(to), true);
        self.touch();
    }

    /// Step the seed on to the next scatter of the same size.
    ///
    /// One undo step however many times it is pressed in a row: shuffling is
    /// looking for a scatter that looks right, and the way back from a search
    /// that found nothing is to where it started, not one seed back.
    pub(crate) fn shuffle_noise(&mut self, id: NodeId) {
        let Some(seed) = self.scene.get(id).and_then(|node| node.params()).map(|params| params.int("noise_seed"))
        else {
            return;
        };
        self.edit("Shuffle noise", Some(&format!("noise-seed:{id}")));
        if let Some(params) = self.scene.get_mut(id).and_then(|node| node.params_mut()) {
            params.insert("noise_seed".to_string(), ParamValue::Count(seed % 9999 + 1));
        }
        self.touch();
    }
}

/// What a fresh pattern holds for one of the scatter's parameters.
fn default_of(key: &str) -> Option<ParamValue> {
    pattern::param_spec(key).map(|param| param.default)
}
