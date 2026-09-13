//! The scatter's builder: a card for each part in use and the chips that add
//! the rest (issue 79).

use super::*;
use crate::app::App;
use crate::panel_properties::{
    field_row, field_row_boxed, number_field, param_field_as, room_left, vector_row, RowStyle,
};
use crate::pattern_tool::{add_chip, length, note};
use crate::theme::{self, token};
use simple3d_core::pattern;
use simple3d_core::primitive::Params;
use simple3d_core::scene::NodeId;

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
