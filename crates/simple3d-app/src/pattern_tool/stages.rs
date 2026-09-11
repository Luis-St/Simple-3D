//! The stages of a custom rule, and what each one reads as.

use super::*;
use crate::app::App;
use crate::theme::{self, token};
use simple3d_core::pattern::{self, StageMode};
use simple3d_core::scene::NodeId;

/// What one of a stage's own controls asked for this frame. Collected and acted
/// on after the stages are drawn, because acting while the loop over them is
/// still running would renumber the very stages it is about to draw.
#[derive(Clone, Copy)]
enum Ask {
    Add,
    Drop(usize),
    Move(usize, bool),
    Fold(usize),
    Vary(usize),
}

/// The rule itself: how many stages, and each stage's numbers.
///
/// A stage is added by a button that says so and dropped by the cross on the
/// stage itself, rather than by a `+` and a `−` beside the count. A pair of
/// signs is the control for a *number*, and the number here is not the thing
/// being edited: what the buttons did was add and remove whole sections of the
/// form below them, and only the count they sat beside said so.
///
/// Any stage can go and any can move (issue 79). A stage repeats what the ones
/// above it made, so dropping one from the middle simply leaves the ones below
/// it repeating what is left -- and the order is the difference between a ring
/// of rows and a row of rings, which is worth two arrows rather than retyping
/// both stages.
pub(crate) fn stages(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let params = app.pattern_tool_params();
    let used = pattern::stage_count(&params);
    let mut ask = None;
    let right = crate::panel_properties::row_right_edge(ui);
    ui.horizontal(|ui| {
        ui.set_max_width((right - ui.max_rect().left()).max(0.0));
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

    let unit = app.unit();
    for index in 0..used {
        let stage = pattern::stage(&params, index);
        // A rule is read as a stack of stages, so each one is ruled off from
        // the one above it: six pixels of air made the whole column one run
        // of rows, and which numbers belonged to which stage had to be
        // worked out from the names.
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(2.0);
        let top = ui.cursor().top();
        let folded = app.pattern_tool_folded[index];
        if let Some(asked) = heading(ui, index, used, folded) {
            ask = Some(asked);
        }
        // What the stage does in a line of English, under its name rather than
        // beside it: beside it, a long one pushed the stage's own buttons off
        // the end of the row.
        ui.add(egui::Label::new(theme::hint(describe(&stage, unit))).selectable(false).wrap());
        if !folded {
            // The axis is a run's only when its copies spin, so it is drawn in
            // the "Vary" section for a run and among the stage's own numbers
            // for everything else.
            let vary = pattern::vary_keys(index);
            let axis = pattern::STAGES[index].axis;
            let own = pattern::stage_keys(index)
                .into_iter()
                .filter(|key| !vary.contains(key) || (*key == axis && stage.mode != StageMode::Move));
            fields(app, ui, id, own, &params);
            if stage.mode != StageMode::Mirror {
                let open = app.pattern_tool_vary_open[index];
                if vary_line(ui, index, open, &stage) {
                    ask = Some(Ask::Vary(index));
                }
                if open {
                    let varied = vary.into_iter().filter(|key| *key != axis || stage.mode == StageMode::Move);
                    fields(app, ui, id, varied, &params);
                }
            }
        }
        // The whole of the stage, heading to last field: the pointer anywhere
        // over it marks what the rule has made by the end of it.
        let block =
            egui::Rect::from_min_max(egui::pos2(ui.min_rect().left(), top), egui::pos2(right, ui.cursor().top()));
        if ui.rect_contains_pointer(block) {
            app.pattern_tool_hover = Some(index);
        }
    }
    // With the rule full there is nothing to add, so the button goes rather
    // than sitting there greyed out -- and its rule goes with it, or the
    // last stage would be underlined by a line with nothing under it. The
    // count in the header already says the rule is full.
    if used < pattern::MAX_STAGES {
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(4.0);
        if ui.button("Add a stage").on_hover_text("Repeat what the stages above it make").clicked() {
            ask = Some(Ask::Add);
        }
    }
    match ask {
        Some(Ask::Add) => app.set_stage_count(used + 1),
        Some(Ask::Drop(index)) => app.drop_stage(index),
        Some(Ask::Move(index, up)) => app.move_stage(index, up),
        Some(Ask::Fold(index)) => app.pattern_tool_folded[index] = !app.pattern_tool_folded[index],
        Some(Ask::Vary(index)) => app.pattern_tool_vary_open[index] = !app.pattern_tool_vary_open[index],
        None => {}
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
    let right = crate::panel_properties::row_right_edge(ui);
    ui.horizontal(|ui| {
        ui.set_max_width((right - ui.max_rect().left()).max(0.0));
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
            // control in the tool that throws work away.
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

/// The line that opens a stage's "Vary" section, with what it holds when shut.
/// Returns whether it was clicked.
fn vary_line(ui: &mut egui::Ui, index: usize, open: bool, stage: &pattern::Stage) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        let (twisty, _) = ui.allocate_exact_size(egui::Vec2::splat(14.0), egui::Sense::hover());
        let name = ui.add(egui::Label::new(theme::value("Vary")).selectable(false).sense(egui::Sense::click()));
        let target = ui.interact(twisty.union(name.rect), vary_id(index), egui::Sense::click());
        let hovered = target.hovered() || name.hovered();
        theme::twisty(ui.painter(), twisty.center(), open, if hovered { token::TEXT_HI } else { token::TEXT_LO });
        // Shut, the line says whether there is anything in it -- a stage whose
        // copies spin and the section folded over the spin reads as a rule that
        // does something nobody can see a number for.
        let what = if stage.varies() { "in use" } else { "gaps, shift, spin, size" };
        ui.add(egui::Label::new(theme::hint(what)).selectable(false));
        clicked = target.clicked() || name.clicked();
    });
    clicked
}

/// The rows for these of a stage's keys that its mode shows.
fn fields(
    app: &mut App,
    ui: &mut egui::Ui,
    id: NodeId,
    keys: impl Iterator<Item = &'static str>,
    params: &simple3d_core::primitive::Params,
) {
    let unit = app.unit();
    for key in keys {
        let Some(spec) = pattern::PARAMS.iter().find(|p| p.key == key) else { continue };
        if !pattern::param_visible(spec, params) {
            continue;
        }
        // Drawn without the stage number the label carries. The label is what
        // a value field answers to across a relayout, so four stages cannot
        // share the word "Copies" -- but the stage they sit under has just been
        // named, and "1 Copies" under "Stage 1" says it twice and reads as
        // neither English nor a number.
        crate::panel_properties::param_field_as(
            app,
            ui,
            &[id],
            id,
            spec,
            without_number(spec.label),
            unit,
            crate::panel_properties::PATTERN_TOOL_ROW,
        );
    }
}

/// A stage parameter's label without the stage number in front of it: "1 Radius
/// per copy" is drawn as "Radius per copy".
pub(crate) fn without_number(label: &'static str) -> &'static str {
    match label.split_once(' ') {
        Some((first, rest)) if first.len() == 1 && first.as_bytes()[0].is_ascii_digit() => rest,
        _ => label,
    }
}

/// A line of English saying what one stage does, so the numbers above it can be
/// read without working them out.
pub(crate) fn describe(stage: &pattern::Stage, unit: simple3d_core::unit::Unit) -> String {
    use simple3d_core::unit::{format_length, format_number};
    let axis = ["X", "Y", "Z"][stage.axis.min(2)];
    if stage.mode == StageMode::Mirror {
        return format!("mirrored across {axis}");
    }
    let length = |mm: f64| format!("{} {}", format_length(mm, unit), unit.suffix());
    let mut what: Vec<String> = Vec::new();
    if stage.mode == StageMode::Turn {
        if stage.turn.abs() > 1e-9 {
            what.push(format!("turning {} deg about {axis}", format_number(stage.turn, 1)));
        }
        if stage.radius.abs() > 1e-9 || stage.growth.abs() > 1e-9 {
            what.push(format!("at radius {}", length(stage.radius)));
        }
        if stage.rise.abs() > 1e-9 {
            what.push(format!("rising {}", length(stage.rise)));
        }
    } else {
        let run = stage.step.length();
        if run > 1e-9 {
            what.push(format!("{} apart", length(run)));
            if stage.gap_growth.abs() > 1e-9 {
                let way = if stage.gap_growth > 0.0 { "widening" } else { "narrowing" };
                what.push(format!("gaps {way} {} a copy", length(stage.gap_growth.abs())));
            }
        }
    }
    // What varies the copies, in the same sentence: a rule that staggers its
    // rows has to say so where the rule is read, or the shift is a number
    // folded away under a heading that claims the rows line up.
    if stage.shift.length() > 1e-9 {
        let cycle = match stage.shift_every {
            2 => "every other copy".to_string(),
            n => format!("over each {n}"),
        };
        what.push(format!("shifting {} {cycle}", length(stage.shift.length())));
    }
    if stage.spin.abs() > 1e-9 {
        what.push(format!("spinning {} deg about {axis}", format_number(stage.spin, 1)));
    }
    if (stage.scale - 1.0).abs() > 1e-9 {
        what.push(format!("each {} % of the last", format_number(stage.scale * 100.0, 0)));
    }
    if what.is_empty() {
        what.push("in place".to_string());
    }
    // A blank stage makes exactly one copy, and a line of English reading
    // "1 copies" draws attention to itself rather than to the stage.
    let copies = stage.copies();
    format!("{copies} cop{}, {}", if copies == 1 { "y" } else { "ies" }, what.join(", "))
}
