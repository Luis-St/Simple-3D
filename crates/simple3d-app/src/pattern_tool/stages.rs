//! The stages of a custom rule, and what each one reads as.

use super::*;
use crate::app::App;
use crate::theme;
use simple3d_core::pattern;
use simple3d_core::scene::NodeId;

/// The rule itself: how many stages, and each stage's numbers.
///
/// A stage is added by a button that says so and dropped by the cross on the
/// stage itself, rather than by a `+` and a `−` beside the count. A pair of
/// signs is the control for a *number*, and the number here is not the thing
/// being edited: what the buttons did was add and remove whole sections of the
/// form below them, and only the count they sat beside said so.
pub(crate) fn stages(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let params = app.pattern_tool_params();
    let used = pattern::stage_count(&params);
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(theme::header_text("Stages")).selectable(false));
        ui.add(egui::Label::new(theme::value(format!("{used} of {}", pattern::MAX_STAGES))).selectable(false));
    });
    ui.add(egui::Label::new(theme::hint("Each stage repeats what the ones above it made.")).selectable(false));

    let unit = app.unit();
    let room = ui.available_height();
    let mut wanted = used;
    egui::ScrollArea::vertical().max_height(room).id_salt("pattern-stages").show(ui, |ui| {
        for index in 0..used {
            let stage = pattern::stage(&params, index);
            // A rule is read as a stack of stages, so each one is ruled off from
            // the one above it: six pixels of air made the whole column one run
            // of rows, and which numbers belonged to which stage had to be
            // worked out from the names.
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(2.0);
            // The stage's name line ends where its own fields do. Left to
            // itself the row runs to the edge of the column, and the cross sat
            // those eight pixels further right than every field under it --
            // which reads as a button that missed the column rather than as one
            // belonging to it. Measured out here, on the ui the field rows are
            // given, because that is the rectangle they are pinned to.
            let right = crate::panel_properties::row_right_edge(ui);
            ui.horizontal(|ui| {
                ui.set_max_width((right - ui.max_rect().left()).max(0.0));
                ui.add(egui::Label::new(theme::header_text(pattern::STAGES[index].label)).selectable(false));
                ui.add(egui::Label::new(theme::hint(describe(&stage, unit))).selectable(false));
                // Only the last stage can go: a stage repeats what the ones
                // above it made, so there is no such thing as removing one from
                // the middle -- the stages below it would be repeating something
                // else. The cross is on the one that can, rather than on all of
                // them refusing.
                if index + 1 == used && used > 1 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let cross =
                            ui.add_sized(egui::Vec2::splat(DROP_STAGE), egui::Button::new("\u{00d7}")).on_hover_text(
                                "Drop this stage. Only the last one can go: the others are what it repeats.",
                            );
                        // Named rather than found by where it sits, so a test can
                        // ask the context where it was drawn -- the bargain the
                        // preview and every grip in the application make. It
                        // senses nothing; the button above answers the pointer.
                        ui.interact(cross.rect, drop_stage_id(), egui::Sense::hover());
                        if cross.clicked() {
                            wanted = used - 1;
                        }
                    });
                }
            });
            for key in pattern::stage_keys(index) {
                let Some(spec) = pattern::PARAMS.iter().find(|p| p.key == key) else { continue };
                if !pattern::param_visible(spec, &params) {
                    continue;
                }
                crate::panel_properties::param_field(
                    app,
                    ui,
                    &[id],
                    id,
                    spec,
                    unit,
                    crate::panel_properties::PATTERN_TOOL_ROW,
                );
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
                wanted = used + 1;
            }
        }
    });
    if wanted != used {
        app.set_stage_count(wanted);
    }
}

/// A line of English saying what one stage does, so the numbers above it can be
/// read without working them out.
pub(crate) fn describe(stage: &pattern::Stage, unit: simple3d_core::unit::Unit) -> String {
    if stage.mirror {
        return format!("mirrored across {}", ["X", "Y", "Z"][stage.axis.min(2)]);
    }
    let mut what: Vec<String> = Vec::new();
    let run = stage.step.length();
    if run > 1e-9 {
        what.push(format!("{} {} apart", simple3d_core::unit::format_length(run, unit), unit.suffix()));
    }
    if stage.turn.abs() > 1e-9 {
        what.push(format!(
            "turning {} deg about {}",
            simple3d_core::unit::format_number(stage.turn, 1),
            ["X", "Y", "Z"][stage.axis.min(2)]
        ));
    }
    if stage.radius.abs() > 1e-9 || stage.growth.abs() > 1e-9 {
        what.push(format!("at radius {} {}", simple3d_core::unit::format_length(stage.radius, unit), unit.suffix()));
    }
    if what.is_empty() {
        what.push("in place".to_string());
    }
    format!("{} copies, {}", stage.copies(), what.join(", "))
}
