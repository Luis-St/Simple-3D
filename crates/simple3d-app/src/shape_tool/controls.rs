//! The function and its numbers.

use super::*;
use crate::app::App;
use crate::popup::{self, label};
use crate::theme;
use simple3d_geom::shaping::{Direction, Function};

/// Scopes fields to this tool, so a drag passed between tool windows cannot edit the wrong number.
const SCOPE: &str = "shape-field";

fn number(app: &mut App, ui: &mut egui::Ui, name: &str, kind: ParamKind, value: &mut f64) {
    popup::number(app, ui, (SCOPE, name), FIELD_WIDTH, kind, value);
}

/// A count field over a `u32`.
fn count(app: &mut App, ui: &mut egui::Ui, name: &str, kind: ParamKind, value: &mut u32) {
    let mut wide = f64::from(*value);
    number(app, ui, name, kind, &mut wide);
    *value = wide.round().max(1.0) as u32;
}

/// Which function, and only the numbers it reads.
pub(crate) fn controls(app: &mut App, ui: &mut egui::Ui, tool: &mut ShapeTool) {
    let length = format!("({})", app.unit().suffix());
    let plan = &mut tool.plan;
    egui::Grid::new("shape-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        label(ui, "Function", "What to run over the surface.");
        egui::ComboBox::from_id_salt("shape-function").selected_text(theme::value(plan.function.label())).show_ui(
            ui,
            |ui| {
                for option in Function::ALL {
                    ui.selectable_value(&mut plan.function, option, option.label()).on_hover_text(hint(option));
                }
            },
        );
        ui.end_row();

        if plan.function != Function::Smooth {
            label(
                ui,
                &format!("Amount {length}"),
                "How far the surface moves at most. Negative pushes it in instead of out.",
            );
            number(app, ui, "amount", AMOUNT, &mut plan.amount);
            ui.end_row();
        }

        if plan.function.patterned() {
            label(
                ui,
                &format!("Size {length}"),
                "How big the pattern's features are: the distance from one bump or cell to the next.",
            );
            number(app, ui, "scale", SCALE, &mut plan.scale);
            ui.end_row();

            label(
                ui,
                "Layers",
                "Layers of finer detail on top of the first, each half the size and half the height of the \
                 one before. One gives a plain pattern, more a rougher, more natural one.",
            );
            count(app, ui, "layers", LAYERS, &mut plan.layers);
            ui.end_row();

            label(ui, "Seed", "Which of the endless patterns of this kind. The same seed always gives the same one.");
            ui.horizontal(|ui| {
                count(app, ui, "seed", SEED, &mut plan.seed);
                if ui.button("Another").on_hover_text("Try the next pattern").clicked() {
                    plan.seed = plan.seed % 99_999 + 1;
                }
            });
            ui.end_row();
        }

        if plan.function == Function::Smooth {
            label(
                ui,
                "Passes",
                "How often the surface is smoothed. Each pass takes out the smallest bumps first; many passes \
                 round off larger shapes too. The volume is kept, so the shape does not shrink.",
            );
            count(app, ui, "passes", PASSES, &mut plan.layers);
            ui.end_row();
        }

        if !matches!(plan.function, Function::Smooth | Function::Inflate) {
            label(ui, "Direction", "Which way the surface moves.");
            egui::ComboBox::from_id_salt("shape-direction")
                .selected_text(theme::value(plan.direction.label()))
                .show_ui(ui, |ui| {
                    for option in Direction::ALL {
                        ui.selectable_value(&mut plan.direction, option, option.label());
                    }
                });
            ui.end_row();
        }

        label(
            ui,
            &format!("Add detail {length}"),
            "Split the mesh first until no edge is longer than this. A converted shape has big flat \
             triangles with too few corners to show anything finer than themselves.",
        );
        ui.horizontal(|ui| {
            popup::optional(app, ui, &mut plan.refine, (SCOPE, "edge"), FIELD_WIDTH, EDGE, &mut plan.edge);
        });
        ui.end_row();

        label(
            ui,
            &format!("Keep the base {length}"),
            "Leave the bottom of the mesh as it is up to this height, and fade the function in over the same \
             height above it, so a print still sits flat on the bed.",
        );
        ui.horizontal(|ui| {
            popup::optional(app, ui, &mut plan.keep_base, (SCOPE, "base"), FIELD_WIDTH, BASE, &mut plan.base);
        });
        ui.end_row();

        label(
            ui,
            "Keep open edges",
            "Leave the rim of a hole exactly where it is, so an open surface keeps its outline.",
        );
        ui.checkbox(&mut plan.keep_boundaries, "");
        ui.end_row();
    });
}

/// What each function does, for its entry in the list.
fn hint(function: Function) -> &'static str {
    match function {
        Function::Noise => "Smooth random bumps, in and out",
        Function::Ridges => "Sharp crests rising out of the surface, as on rock",
        Function::Cells => "Rounded cells with creases between them, as on hammered metal",
        Function::Smooth => "Take bumps out without shrinking the shape",
        Function::Inflate => "Push the whole surface out, or in, by the same distance",
    }
}
