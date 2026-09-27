//! The numbers a cut is described by.

use super::*;
use crate::app::App;
use crate::theme;
use simple3d_core::primitive::ParamKind;
use simple3d_geom::tiling::{CellKind, Tiling};

/// Every cut in the plan, and adding or dropping one.
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
        // The header names the cut only when there are several; the buttons show either way.
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
                // Reset the numbers a new cut starts with (stock size, no turn, offset or layers), keeping the
                // cell shape and axis, which are choices rather than numbers.
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
    // A field being typed into holds its own text, which would overwrite the reset numbers.
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
        // A new cut starts on the next axis, since a second tiling on the same axis would be redundant.
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

/// The painted cross that drops one cut, as the UI font may lack the glyph.
pub(crate) fn drop_button(ui: &mut egui::Ui, index: usize) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(14.0), egui::Sense::click());
    let colour = if response.hovered() { theme::token::DANGER } else { theme::token::TEXT_LO };
    let arm = rect.shrink(3.5);
    let stroke = egui::Stroke::new(1.4_f32, colour);
    ui.painter().line_segment([arm.left_top(), arm.right_bottom()], stroke);
    ui.painter().line_segment([arm.right_top(), arm.left_bottom()], stroke);
    // Painted, so it needs an accessible name.
    let label = format!("Drop cut {}", index + 1);
    let name = label.clone();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    response.on_hover_text(&label).clicked()
}

/// One cut: its cell shape, size and direction.
pub(crate) fn pass(app: &mut App, ui: &mut egui::Ui, index: usize, tiling: &mut Tiling) {
    // Every field names its unit, like the properties panel.
    let length = format!("({})", app.unit().suffix());
    // Cell shapes in their own row above the grid, since four chips wrapping inside an `egui::Grid`
    // overlapped the next row.
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

        // The axis the cells run along: a flat plate is cut into columns standing up it, which is Z.
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
        number(
            app,
            ui,
            &field_name(index, "angle"),
            ParamKind::Angle { min: -360.0, max: 360.0, wrap: false },
            &mut tiling.angle,
        );
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

        // "Layer height", since "Layers" with a 4 reads as four layers.
        label(
            ui,
            &format!("Layer height {length}"),
            "Cut across the cells as well, into layers this tall. Zero cuts straight through.",
        );
        number(app, ui, &field_name(index, "layer"), ParamKind::Length { min: 0.0 }, &mut tiling.layer);
        ui.end_row();
    });
}
