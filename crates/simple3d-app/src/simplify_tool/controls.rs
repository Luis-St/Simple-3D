//! The numbers and the guards a simplification is described by.

use super::*;
use crate::app::App;
use crate::theme;

/// How much to drop, and what may not be dropped on the way.
pub(crate) fn controls(app: &mut App, ui: &mut egui::Ui, tool: &mut SimplifyTool) {
    let length = format!("({})", app.unit().suffix());
    egui::Grid::new("simplify-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        label(
            ui,
            "Detail (%)",
            "How much of the mesh to keep, as a percentage of its triangles. The ones that go are the ones \
             whose loss moves the surface least, not every nth one.",
        );
        let mut detail = f64::from(tool.plan.detail);
        number(app, ui, "detail", DETAIL, &mut detail);
        tool.plan.detail = detail.round().clamp(1.0, 100.0) as u32;
        ui.end_row();

        label(
            ui,
            &format!("Keep within {length}"),
            "Stop before the surface moves further than this, whatever the percentage still asks for. \
             This is the number a part that has to fit something else is measured by.",
        );
        ui.horizontal(|ui| {
            optional(app, ui, &mut tool.plan.limit_deviation, "deviation", DEVIATION, &mut tool.plan.max_deviation);
        });
        ui.end_row();

        label(
            ui,
            "Keep sharp edges",
            "Never collapse across a crease sharper than this, so corners, rims and chamfers stay \
             exactly where they are. A curve tessellated more finely than the angle is not a crease.",
        );
        ui.horizontal(|ui| {
            optional(app, ui, &mut tool.plan.keep_sharp, "sharp", SHARP, &mut tool.plan.sharp_angle);
            ui.label(theme::hint("deg"));
        });
        ui.end_row();

        label(
            ui,
            "Keep open edges",
            "Leave the rim of a hole exactly where it is. A mesh that is not closed has nothing on the far \
             side of its boundary to hold the surface in place, so a rim left free creeps inwards.",
        );
        ui.checkbox(&mut tool.plan.keep_boundaries, "");
        ui.end_row();

        label(
            ui,
            "Keep colour seams",
            "Leave the line between two painted surfaces where it is. A colour is carried on the triangle, \
             so a collapse across that line moves the paint as well as the surface.",
        );
        ui.checkbox(&mut tool.plan.keep_colours, "");
        ui.end_row();

        label(
            ui,
            "Show the triangles",
            "Draw the triangles of the result over the shape. Most of what a simplification does shows \
             in the triangulation long before it shows in the outline.",
        );
        ui.checkbox(&mut tool.wireframe, "");
        ui.end_row();
    });
}
