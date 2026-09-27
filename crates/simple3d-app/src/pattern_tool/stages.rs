//! The stages of a custom rule, built up card by card (issue 79).
//!
//! A builder rather than a form: each stage is a card with its mode chips, the numbers that mode
//! needs, and its variations as removable sub-cards. Variations are added by kind, which lets a
//! stage hold several; nothing unused is shown.

use super::*;
use crate::app::App;
use crate::panel_properties::{field_row_boxed, param_field_as, vector_row, PATTERN_TOOL_ROW};
use crate::theme::{self, token};
use simple3d_core::pattern::{self, StageMode, Vary};
use simple3d_core::primitive::Params;
use simple3d_core::scene::NodeId;

/// A builder control's request this frame, applied after drawing so stages are not renumbered
/// mid-loop.
#[derive(Clone, Copy)]
enum Ask {
    Add(StageMode),
    Drop(usize),
    Move(usize, bool),
    Fold(usize),
    AddVariation(usize, Vary),
    DropVariation(usize, usize),
}

/// What "Add a stage" offers, in order, with descriptions.
const ADD_STAGE: [(StageMode, &str, &str); 3] = [
    (StageMode::Move, "+ Move", "A run: each copy a fixed step further along"),
    (StageMode::Turn, "+ Turn", "A ring, a helix or a spiral: each copy turned further about an axis"),
    (StageMode::Mirror, "+ Mirror", "Everything the stages above made, and its reflection"),
];

/// The rule: a card per stage and a row adding the next. Any stage can be removed or moved;
/// order matters (a ring of rows versus a row of rings).
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
        // Hovering anywhere on the card marks what the rule has made by its end, and outlines the card.
        let rect = drawn.response.rect;
        if ui.rect_contains_pointer(rect) {
            app.pattern_tool_hover = Some(index);
            let stroke = egui::Stroke::new(1.0_f32, token::ACCENT.gamma_multiply(0.7));
            ui.painter().rect_stroke(rect, CARD_ROUNDING, stroke, egui::StrokeKind::Inside);
        }
    }
    // With the rule full the add row is hidden rather than greyed out.
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

/// One stage's card: heading, description, mode chips with their numbers, and variations.
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
    // Under the name rather than beside it, where a long line pushed the buttons off the row.
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
    // A mirror is two copies, with nothing between them to vary.
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
    // Chips for the variation kinds the stage still has room for; hidden when none is left.
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

/// A stage's heading: twisty and name to fold it, arrows and cross to move and drop it.
/// Measured to end where the fields do, so the buttons line up with the column.
fn heading(ui: &mut egui::Ui, index: usize, used: usize, folded: bool) -> Option<Ask> {
    let mut ask = None;
    capped_row(ui, |ui| {
        let (twisty, _) = ui.allocate_exact_size(egui::Vec2::splat(14.0), egui::Sense::hover());
        let name = ui.add(
            egui::Label::new(theme::header_text(pattern::STAGES[index].label))
                .selectable(false)
                .sense(egui::Sense::click()),
        );
        // The twisty and name are one target, since the triangle alone is too small.
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
            // Square and twice egui's small button height: marks rather than words.
            let square = egui::Vec2::splat(DROP_STAGE);
            let cross = ui
                .add_sized(square, egui::Button::new("\u{00d7}"))
                .on_hover_text("Drop this stage. The ones below it move up and repeat what is left above them.");
            // A named id so tests can find where it was drawn; the button handles the pointer.
            ui.interact(cross.rect, drop_stage_id(index), egui::Sense::hover());
            if cross.clicked() {
                ask = Some(Ask::Drop(index));
            }
            for (up, enabled, hover) in [
                (false, index + 1 < used, "Move this stage down: it repeats the one below it instead"),
                (true, index > 0, "Move this stage up: the one above it repeats it instead"),
            ] {
                // Painted, since the UI font has no small triangles and would draw empty boxes.
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

/// One of the rule's numbers or choices, under the builder's name for it. The parameter's own
/// label carries the stage number ("2.1 Spin") so value fields stay stable across relayout.
pub(super) fn field(app: &mut App, ui: &mut egui::Ui, id: NodeId, key: &str, name: &str, params: &Params) {
    let Some(spec) = pattern::param_spec(key) else { return };
    if !pattern::param_visible(spec, params) {
        return;
    }
    let unit = app.unit();
    param_field_as(app, ui, &[id], id, spec, name, unit, PATTERN_TOOL_ROW);
}
