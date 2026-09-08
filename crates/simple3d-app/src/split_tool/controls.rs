//! The numbers a cut is described by.

use super::*;
use crate::app::App;
use crate::theme;
use simple3d_core::primitive::ParamKind;
use simple3d_geom::tiling::{CellKind, Tiling};

/// Every cut in the plan, and the way to add or drop one.
pub(crate) fn controls(app: &mut App, ui: &mut egui::Ui, tool: &mut SplitTool) {
    let cuts = tool.plan.passes.len();
    let mut drop = None;
    let mut reset = None;
    for (index, tiling) in tool.plan.passes.iter_mut().enumerate() {
        if index > 0 {
            ui.add_space(4.0);
            ui.separator();
            ui.add_space(2.0);
        }
        // The header names the cut only when there is more than one: a window
        // that says "Cut 1" over a single cut is answering a question nobody
        // had. The buttons on the right of it are there either way.
        ui.horizontal(|ui| {
            if cuts > 1 {
                ui.label(
                    egui::RichText::new(format!("Cut {}", index + 1))
                        .size(theme::font::LABEL)
                        .color(theme::token::TEXT_HI),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if cuts > 1 && drop_button(ui, index) {
                    drop = Some(index);
                }
                // Back to the numbers a cut starts with: no turn, no offset, no
                // layers, and the stock cell size. Not literally every number
                // to zero -- a cell of no size is a split that is refused, so
                // the size goes back to the one a new cut opens on -- and not
                // the cell shape or the axis, which are choices rather than
                // numbers and are the two things worth keeping while the
                // numbers are thrown away.
                if ui
                    .button("Reset")
                    .on_hover_text(
                        "Put this cut's numbers back: no turn, no offset, no layers, and the cell size a new \
                         cut opens on. The cell shape and the axis are left as they are.",
                    )
                    .clicked()
                {
                    *tiling = Tiling { kind: tiling.kind, axis: tiling.axis, ..Tiling::default() };
                    reset = Some(index);
                }
            });
        });
        pass(app, ui, index, tiling);
    }
    // A field being typed into holds its own text until it is left, and that
    // text is what would be read back over the numbers this just put right.
    if let Some(index) = reset {
        for part in ["size", "depth", "angle", "layer", "offset-0", "offset-1"] {
            app.fields.forget(egui::Id::new(("split-field", field_name(index, part))));
        }
    }
    if let Some(index) = drop {
        tool.plan.passes.remove(index);
    }
    if tool.plan.passes.len() < simple3d_geom::tiling::MAX_PASSES {
        ui.add_space(6.0);
        // The new cut starts on the next axis round rather than on the one
        // already being cut: two identical tilings on the same axis are one
        // tiling, so the useful second cut is the one across the first.
        let next =
            tool.plan.passes.last().map_or_else(Tiling::default, |last| Tiling { axis: (last.axis + 1) % 3, ..*last });
        if ui
            .button("Add another cut")
            .on_hover_text(
                "Cut the pieces again, on another axis or in another shape. The pieces are what both cuts \
                 leave -- a plate scored into squares and then into slabs comes back as blocks.",
            )
            .clicked()
        {
            tool.plan.passes.push(next);
        }
    }
}

/// The cross that drops one cut, drawn rather than written.
///
/// The same two strokes the popup's own close cross is, for the same reason: a
/// cross typed as a character is a character the interface font may not have,
/// and the one it puts in its place is an empty box.
pub(crate) fn drop_button(ui: &mut egui::Ui, index: usize) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(14.0), egui::Sense::click());
    let colour = if response.hovered() { theme::token::DANGER } else { theme::token::TEXT_LO };
    let arm = rect.shrink(3.5);
    let stroke = egui::Stroke::new(1.4_f32, colour);
    ui.painter().line_segment([arm.left_top(), arm.right_bottom()], stroke);
    ui.painter().line_segment([arm.right_top(), arm.left_bottom()], stroke);
    // Painted, so nothing would otherwise say what it is: to anything reading
    // the interface it was an unnamed rectangle.
    let label = format!("Drop cut {}", index + 1);
    let name = label.clone();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    response.on_hover_text(&label).clicked()
}

/// One cut of the plan: what shape its cells are, how big, and where they run.
pub(crate) fn pass(app: &mut App, ui: &mut egui::Ui, index: usize, tiling: &mut Tiling) {
    // Every field says what it is measured in, the way the properties panel's
    // rows do: a number in a box is a number in some unit, and which one is not
    // something to work out from the document setting three panels away.
    let length = format!("({})", app.unit().suffix());
    // The cell shapes are a row of their own above the grid rather than a cell
    // in it. Four chips do not fit across the width of a popup, and an
    // `egui::Grid` does not grow its row for a wrapped one: the fourth landed
    // on top of the row below, which is the Size field.
    ui.label(egui::RichText::new("Cells").size(theme::font::LABEL).color(theme::token::TEXT_LO));
    ui.horizontal_wrapped(|ui| {
        for kind in CellKind::ALL {
            if theme::choice(ui, tiling.kind == kind, kind.label()).on_hover_text(kind.size_meaning()).clicked() {
                tiling.kind = kind;
            }
        }
    });
    ui.add_space(6.0);
    egui::Grid::new(("split-grid", index)).num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        label(ui, &format!("Size {length}"), tiling.kind.size_meaning());
        number(app, ui, &field_name(index, "size"), SIZE, &mut tiling.size);
        ui.end_row();

        if tiling.kind.has_depth() {
            label(ui, &format!("Depth {length}"), "The second side of one rectangle.");
            number(app, ui, &field_name(index, "depth"), SIZE, &mut tiling.depth);
            ui.end_row();
        }

        // The axis is the direction the cells *run in*, not the plane they lie
        // in, which is the way round a cut is thought about: a plate lying flat
        // is cut into columns standing up it, which is Z.
        ui.label("Through");
        ui.horizontal(|ui| {
            for (axis, name) in [(0u8, "X"), (1, "Y"), (2, "Z")] {
                if theme::choice(ui, tiling.axis == axis, name).clicked() {
                    tiling.axis = axis;
                }
            }
        })
        .response
        .on_hover_text("The axis the cells run along. The tiling lies in the plane across it.");
        ui.end_row();

        label(ui, "Turn (deg)", "Turn the whole grid within its plane, in degrees.");
        number(app, ui, &field_name(index, "angle"), ParamKind::Angle { min: -360.0, max: 360.0 }, &mut tiling.angle);
        ui.end_row();

        label(
            ui,
            &format!("Offset {length}"),
            "Move the grid within its plane. The cells are centred on the shape until this says otherwise.",
        );
        ui.horizontal(|ui| {
            for i in 0..2 {
                let name = field_name(index, if i == 0 { "offset-0" } else { "offset-1" });
                number(app, ui, &name, ParamKind::Length { min: f64::NEG_INFINITY }, &mut tiling.offset[i]);
            }
        });
        ui.end_row();

        // "Layer height" rather than "Layers": the number is how tall one layer
        // is, and a row called Layers holding a 4 reads as four of them.
        label(
            ui,
            &format!("Layer height {length}"),
            "Cut across the cells as well, into layers this tall. Zero cuts straight through.",
        );
        number(app, ui, &field_name(index, "layer"), ParamKind::Length { min: 0.0 }, &mut tiling.layer);
        ui.end_row();
    });
}
