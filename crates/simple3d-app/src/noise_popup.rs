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

/// The window's contents: the scatter's builder.
pub(crate) fn body(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    builder(app, ui, id, PATTERN_ROW);
}

/// The three parts a scatter is built from, in the order the builder lists
/// them (issue 79).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    /// Moved off where the rule put it, along each axis.
    Nudge,
    /// Turned where it stands.
    Turn,
    /// Made bigger or smaller.
    Size,
}

impl Part {
    pub(crate) const ALL: [Part; 3] = [Part::Nudge, Part::Turn, Part::Size];

    fn index(self) -> usize {
        match self {
            Part::Nudge => 0,
            Part::Turn => 1,
            Part::Size => 2,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Part::Nudge => "Nudge",
            Part::Turn => "Turn",
            Part::Size => "Size",
        }
    }

    fn hover(self) -> &'static str {
        match self {
            Part::Nudge => "Move each copy a little off where the rule puts it",
            Part::Turn => "Turn each copy a little askew where it stands",
            Part::Size => "Make each copy a little bigger or smaller",
        }
    }

    /// The numbers it is made of, which are what taking it off puts back to
    /// nothing. The turn's axis is not one: it says what a turn is about, and
    /// a turn of nothing about Z is no turn at all.
    fn keys(self) -> &'static [&'static str] {
        match self {
            Part::Nudge => &["noise_x", "noise_y", "noise_z"],
            Part::Turn => &["noise_turn"],
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
                part.name(),
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
                Part::Turn => {
                    field(app, ui, id, "noise_turn", "Up to", style);
                    field(app, ui, id, "noise_axis", "About", style);
                }
                Part::Size => field(app, ui, id, "noise_scale", "Up to \u{00B1} (%)", style),
            }
        });
    }
    if shown.len() < Part::ALL.len() {
        ui.add_space(4.0);
        field_row_boxed(ui, "Add", "Let the copies wander off where the rule puts them", |ui| {
            for part in Part::ALL.into_iter().filter(|part| !shown.contains(part)) {
                let chip = theme::choice(ui, false, &format!("+ {}", part.name())).on_hover_text(part.hover());
                // It senses nothing; the chip answers the pointer.
                ui.interact(chip.rect, part_id(scope, part), egui::Sense::hover());
                if chip.clicked() {
                    ask = Some(Ask::Add(part));
                }
            }
        });
    }
    if shown.is_empty() {
        ui.add(
            egui::Label::new(theme::hint(
                "None: every copy is exactly where the rule puts it. Add a part and the copies wander off it by no \
                 more than you say -- planks laid a little askew, stones along a path.",
            ))
            .selectable(false)
            .wrap(),
        );
    } else {
        ui.add_space(4.0);
        seed_row(app, ui, id, style, &mut ask);
        if let Some(spec) = pattern::PARAMS.iter().find(|param| param.key == "noise_keep_first") {
            param_field_as(app, ui, &[id], id, spec, "Keep the original", unit, style);
        }
        ui.add(
            egui::Label::new(theme::hint(
                "Every copy lands somewhere inside this, and stays there: the same seed scatters the same way every \
                 time the file is opened.",
            ))
            .selectable(false)
            .wrap(),
        );
        crowding_note(app, ui, id, &params);
    }
    match ask {
        Some(Ask::Add(part)) => app.add_noise_part(id, part),
        Some(Ask::Drop(part)) => app.drop_noise_part(id, part),
        Some(Ask::Shuffle) => app.shuffle_noise(id),
        None => {}
    }
}

/// The seed, and the button that steps it on: the same amounts, scattered
/// another way. Typing numbers into a seed to find a scatter that looks right
/// is clicking a button with extra steps.
fn seed_row(app: &mut App, ui: &mut egui::Ui, id: NodeId, style: RowStyle, ask: &mut Option<Ask>) {
    let Some(spec) = pattern::PARAMS.iter().find(|param| param.key == "noise_seed") else { return };
    let unit = app.unit();
    let scope = style.scope();
    field_row(
        ui,
        "Seed",
        "The same seed scatters the same way every time the file is opened; another is another scatter of the \
         same size",
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
    let Some(spec) = pattern::PARAMS.iter().find(|param| param.key == key) else { return };
    let unit = app.unit();
    param_field_as(app, ui, &[id], id, spec, name, unit, style);
}

/// What one part currently comes to, in a few words.
fn summary(part: Part, params: &Params, unit: simple3d_core::unit::Unit) -> String {
    use simple3d_core::unit::{format_length, format_number};
    if !part.in_use(params) {
        return "nothing yet".to_string();
    }
    let scatter = pattern::Noise::of(params);
    match part {
        Part::Nudge => {
            let most = scatter.offset.x.max(scatter.offset.y).max(scatter.offset.z);
            format!("up to {} {} either way", format_length(most, unit), unit.suffix())
        }
        Part::Turn => {
            let about = ["X", "Y", "Z", "all three axes"][scatter.axis.min(3)];
            format!("up to {} deg about {about}", format_number(scatter.turn, 1))
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
    let length = |mm: f64| format!("{} {}", simple3d_core::unit::format_length(mm, unit), unit.suffix());
    ui.add(
        egui::Label::new(
            egui::RichText::new(format!(
                "Copies may meet: the rule leaves {} between neighbours, and two of them wandering towards each other \
                 can close {}. Copies that touch are welded into one body.",
                length(crowded.gap),
                length(crowded.reach)
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
        self.noise_parts_open = (None, [false; 3]);
        self.touch();
    }

    /// Whether the builder is keeping `part` on screen for `id` although it is
    /// at nothing.
    fn noise_part_open(&self, id: NodeId, part: Part) -> bool {
        self.noise_parts_open.0 == Some(id) && self.noise_parts_open.1[part.index()]
    }

    fn set_noise_part_open(&mut self, id: NodeId, part: Part, open: bool) {
        if self.noise_parts_open.0 != Some(id) {
            self.noise_parts_open = (Some(id), [false; 3]);
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
                Part::Turn => {
                    params.insert("noise_turn".to_string(), ParamValue::Angle(3.0));
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
        if let Some(params) = self.scene.get_mut(id).and_then(|node| node.params_mut()) {
            for key in part.keys() {
                if let Some(default) = default_of(key) {
                    params.insert(key.to_string(), default);
                }
            }
        }
        self.set_noise_part_open(id, part, false);
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
fn default_of(key: &str) -> Option<simple3d_core::primitive::ParamValue> {
    pattern::PARAMS.iter().find(|param| param.key == key).map(|param| param.default)
}
