//! The stages of a custom rule, built up card by card (issue 79).
//!
//! The tool is a builder rather than a form. A stage is a card: what it does,
//! chosen from three chips, the few numbers that choice needs, and then the
//! variations on it, each a card of its own inside the stage with a cross that
//! takes it off again. Nothing is on screen that the rule does not use. A
//! variation is *added*, from a row of the four kinds, rather than being a
//! "Vary" section of eight fields that are all zero until one is not -- which
//! is also what lets a stage hold more than one of them: two shifts on two
//! cycles, a spin that builds up and one that flips every other copy back. A
//! stage is added the same way, as the thing it is to do.

use super::*;
use crate::app::App;
use crate::panel_properties::{field_row_boxed, param_field_as, vector_row, PATTERN_TOOL_ROW};
use crate::theme::{self, token};
use simple3d_core::pattern::{self, StageMode, Vary};
use simple3d_core::primitive::Params;
use simple3d_core::scene::NodeId;

/// What one of the builder's controls asked for this frame. Collected and acted
/// on after the stages are drawn, because acting while the loop over them is
/// still running would renumber the very stages it is about to draw.
#[derive(Clone, Copy)]
enum Ask {
    Add(StageMode),
    Drop(usize),
    Move(usize, bool),
    Fold(usize),
    AddVariation(usize, Vary),
    DropVariation(usize, usize),
}

/// What "Add a stage" offers, in order, and what each one is for.
const ADD_STAGE: [(StageMode, &str, &str); 3] = [
    (StageMode::Move, "+ Move", "A run: each copy a fixed step further along"),
    (StageMode::Turn, "+ Turn", "A ring, a helix or a spiral: each copy turned further about an axis"),
    (StageMode::Mirror, "+ Mirror", "Everything the stages above made, and its reflection"),
];

/// The rule itself: a card for each stage, and the row that adds the next.
///
/// Any stage can go and any can move. A stage repeats what the ones above it
/// made, so dropping one from the middle simply leaves the ones below it
/// repeating what is left -- and the order is the difference between a ring of
/// rows and a row of rings, which is worth two arrows rather than retyping both
/// stages.
pub(crate) fn stages(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let params = app.pattern_tool_params();
    let used = pattern::stage_count(&params);
    let mut ask = None;
    capped_row(ui, |ui| {
        ui.add(egui::Label::new(theme::header_text("Stages")).selectable(false));
        ui.add(egui::Label::new(theme::value(format!("{used} of {}", pattern::MAX_STAGES))).selectable(false));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("Start over")
                .on_hover_text("Ask again what the rule starts from. Nothing changes until that is answered.")
                .clicked()
            {
                app.start_rule_over();
            }
        });
    });
    ui.add(
        egui::Label::new(theme::hint(
            "Each stage repeats what the ones above it made. Point at one to see its copies in the viewport.",
        ))
        .selectable(false),
    );

    for index in 0..used {
        ui.add_space(6.0);
        let drawn = card(token::SURFACE_0B).show(ui, |ui| {
            ui.set_width(ui.available_width());
            stage_card(app, ui, id, index, used, &params, &mut ask);
        });
        // The whole card, heading to last field: the pointer anywhere over it
        // marks what the rule has made by the end of it -- and the card is
        // outlined as well, so the marks in the viewport and the stage they
        // belong to are seen together.
        let rect = drawn.response.rect;
        if ui.rect_contains_pointer(rect) {
            app.pattern_tool_hover = Some(index);
            let stroke = egui::Stroke::new(1.0_f32, token::ACCENT.gamma_multiply(0.7));
            ui.painter().rect_stroke(rect, CARD_ROUNDING, stroke, egui::StrokeKind::Inside);
        }
    }
    // With the rule full there is nothing to add, so the row goes rather than
    // sitting there greyed out. The count in the header already says the rule
    // is full.
    if used < pattern::MAX_STAGES {
        ui.add_space(6.0);
        field_row_boxed(ui, "Add a stage", "Repeat everything the stages above make", |ui| {
            for (mode, text, hover) in ADD_STAGE {
                if add_chip(ui, text, hover, add_stage_id(mode)) {
                    ask = Some(Ask::Add(mode));
                }
            }
        });
    }
    match ask {
        Some(Ask::Add(mode)) => app.add_stage_doing(mode),
        Some(Ask::Drop(index)) => app.drop_stage(index),
        Some(Ask::Move(index, up)) => app.move_stage(index, up),
        Some(Ask::Fold(index)) => app.pattern_tool_folded[index] = !app.pattern_tool_folded[index],
        Some(Ask::AddVariation(index, what)) => app.add_variation(index, what),
        Some(Ask::DropVariation(index, slot)) => app.drop_variation(index, slot),
        None => {}
    }
}

/// One stage's card: its heading, what it does in a line of English, what it
/// does as chips and the numbers that needs, and then its variations.
fn stage_card(
    app: &mut App,
    ui: &mut egui::Ui,
    id: NodeId,
    index: usize,
    used: usize,
    params: &Params,
    ask: &mut Option<Ask>,
) {
    let stage = pattern::stage(params, index);
    let unit = app.unit();
    let folded = app.pattern_tool_folded[index];
    if let Some(asked) = heading(ui, index, used, folded) {
        *ask = Some(asked);
    }
    // What the stage does in a line of English, under its name rather than
    // beside it: beside it, a long one pushed the stage's own buttons off the
    // end of the row.
    note(ui, describe(&stage, unit));
    if folded {
        return;
    }
    let k = &pattern::STAGES[index];
    field(app, ui, id, k.mode, "Does", params);
    match stage.mode {
        StageMode::Move => {
            field(app, ui, id, k.count, "Copies", params);
            vector_row(app, ui, id, k.step, "Step", "How far each copy is from the one before it", PATTERN_TOOL_ROW);
        }
        StageMode::Turn => {
            for (key, name) in [
                (k.count, "Copies"),
                (k.axis, "Axis"),
                (k.turn, "Turn per copy"),
                (k.radius, "Radius"),
                (k.growth, "Radius per copy"),
                (k.rise, "Rise per copy"),
            ] {
                field(app, ui, id, key, name, params);
            }
        }
        StageMode::Mirror => field(app, ui, id, k.axis, "Across", params),
    }
    // A mirror is two copies, and there is nothing between two copies to vary.
    if stage.mode == StageMode::Mirror {
        return;
    }
    for slot in 0..stage.variations().len() {
        ui.add_space(4.0);
        card(token::SURFACE_1).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if variation_card(app, ui, id, index, slot, &stage, params) {
                *ask = Some(Ask::DropVariation(index, slot));
            }
        });
    }
    // The chips for the kinds the stage still has room for: one of each axis,
    // way of stepping and set of copies it can reach. With none left the row
    // goes, as the stages' own does when the rule is full.
    let room: Vec<Vary> =
        Vary::ALL.into_iter().filter(|what| what.fits(stage.mode) && pattern::has_room_for(&stage, *what)).collect();
    if !room.is_empty() {
        ui.add_space(4.0);
        field_row_boxed(
            ui,
            "Vary",
            "Change the copies from one to the next, beyond where the stage puts them. A stage takes one of each \
             kind for every axis and set of copies it can reach.",
            |ui| {
                for what in room {
                    if add_chip(ui, &format!("+ {}", what.name()), vary_hover(what), add_variation_id(index, what)) {
                        *ask = Some(Ask::AddVariation(index, what));
                    }
                }
            },
        );
    }
}

/// A stage's heading: the twisty and name that fold it, and the arrows and
/// cross that move and drop it.
///
/// The heading ends where the stage's own fields do. Left to itself the row
/// runs to the edge of the column, and a button on the end of it sat eight
/// pixels further right than every field under it -- which reads as a button
/// that missed the column rather than as one belonging to it. Measured out
/// here, on the ui the field rows are given, because that is the rectangle
/// they are pinned to.
fn heading(ui: &mut egui::Ui, index: usize, used: usize, folded: bool) -> Option<Ask> {
    let mut ask = None;
    capped_row(ui, |ui| {
        let (twisty, _) = ui.allocate_exact_size(egui::Vec2::splat(14.0), egui::Sense::hover());
        let name = ui.add(
            egui::Label::new(theme::header_text(pattern::STAGES[index].label))
                .selectable(false)
                .sense(egui::Sense::click()),
        );
        // The twisty and the name are one target: a fold that only answered to
        // a fourteen-pixel triangle would be a target for nobody.
        let target = ui.interact(twisty.union(name.rect), fold_stage_id(index), egui::Sense::click());
        let hovered = target.hovered() || name.hovered();
        theme::twisty(ui.painter(), twisty.center(), !folded, if hovered { token::TEXT_HI } else { token::TEXT_LO });
        if target.clicked() || name.clicked() {
            ask = Some(Ask::Fold(index));
        }
        if used < 2 {
            return;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Twice the height egui's small button comes out at, and square:
            // these are marks rather than words, and the cross is the one
            // control in the tool that throws a whole stage away.
            let square = egui::Vec2::splat(DROP_STAGE);
            let cross = ui
                .add_sized(square, egui::Button::new("\u{00d7}"))
                .on_hover_text("Drop this stage. The ones below it move up and repeat what is left above them.");
            // Named rather than found by where it sits, so a test can ask the
            // context where it was drawn. It senses nothing; the button
            // answers the pointer.
            ui.interact(cross.rect, drop_stage_id(index), egui::Sense::hover());
            if cross.clicked() {
                ask = Some(Ask::Drop(index));
            }
            for (up, enabled, hover) in [
                (false, index + 1 < used, "Move this stage down: it repeats the one below it instead"),
                (true, index > 0, "Move this stage up: the one above it repeats it instead"),
            ] {
                // Painted rather than typed: the interface face has no small
                // up and down triangles, and a glyph it lacks draws as an empty
                // box -- two blank buttons beside the cross.
                let arrow = ui.add_enabled(enabled, egui::Button::new("").min_size(square)).on_hover_text(hover);
                let colour = if !enabled {
                    token::TEXT_LO.gamma_multiply(0.4)
                } else if arrow.hovered() {
                    token::TEXT_HI
                } else {
                    token::TEXT_LO
                };
                let (c, r) = (arrow.rect.center(), 4.0);
                let (tip, base) = if up { (-r * 0.6, r * 0.6) } else { (r * 0.6, -r * 0.6) };
                ui.painter().add(egui::Shape::convex_polygon(
                    vec![egui::pos2(c.x - r, c.y + base), egui::pos2(c.x + r, c.y + base), egui::pos2(c.x, c.y + tip)],
                    colour,
                    egui::Stroke::NONE,
                ));
                ui.interact(arrow.rect, move_stage_id(index, up), egui::Sense::hover());
                if arrow.clicked() {
                    ask = Some(Ask::Move(index, up));
                }
            }
        });
    });
    ask
}

/// One of the rule's numbers or choices, under the name the builder gives it,
/// where the stage's mode shows it.
///
/// The name is the builder's rather than the parameter's own. The parameter's
/// label is what a value field answers to across a relayout, so four stages
/// with a "Copies" each must carry a number -- "1 Copies", "2.1 Spin" -- that
/// the card they sit on has already said.
pub(super) fn field(app: &mut App, ui: &mut egui::Ui, id: NodeId, key: &str, name: &str, params: &Params) {
    let Some(spec) = pattern::param_spec(key) else { return };
    if !pattern::param_visible(spec, params) {
        return;
    }
    let unit = app.unit();
    param_field_as(app, ui, &[id], id, spec, name, unit, PATTERN_TOOL_ROW);
}
