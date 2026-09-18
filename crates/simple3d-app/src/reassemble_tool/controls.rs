//! The numbers a reassembly is described by.

use super::*;
use crate::app::App;
use crate::popup::{label, number};

/// Which fields are this tool's, so a drag handed between two tools' windows
/// cannot edit the wrong number.
const SCOPE: &str = "reassemble-field";

/// How closely to look, how many objects to make, and what to do with what
/// touches.
pub(crate) fn controls(app: &mut App, ui: &mut egui::Ui, tool: &mut ReassembleTool) {
    let length = format!("({})", app.unit().suffix());
    egui::Grid::new("reassemble-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        label(
            ui,
            "Recognise shapes",
            "Try to name each body: a box, a cylinder, a prism, a cone or a sphere, with its own \
             parameters back. With this off the mesh is only separated into the bodies it is in, \
             which is worth having on its own for parts that were never primitives.",
        );
        ui.checkbox(&mut tool.plan.recognise, "");
        ui.end_row();

        label(
            ui,
            &format!("Fit within {length}"),
            "How far a body's surface may sit from the shape fitted to it and still be called that \
             shape. Tighter finds fewer shapes and is never wrong about the ones it finds; looser \
             recovers a shape that has been through a file format at the cost of calling something \
             round a cylinder.",
        );
        ui.add_enabled_ui(tool.plan.recognise, |ui| {
            ui.horizontal(|ui| {
                number(app, ui, (SCOPE, "tolerance"), FIELD_WIDTH, TOLERANCE, &mut tool.plan.tolerance);
            });
        });
        ui.end_row();

        label(
            ui,
            "Group what touches",
            "Put bodies that touch into a group together. Two solids in contact were an assembly \
             before somebody flattened them, and a group says so -- it moves, paints and exports as \
             one. Bodies standing apart are left apart.",
        );
        ui.checkbox(&mut tool.plan.group_touching, "");
        ui.end_row();

        label(
            ui,
            "At most (objects)",
            "Stop after this many objects. A printed lattice is forty thousand separate bodies, and \
             forty thousand nodes is not a document anybody can work in. The biggest bodies are the \
             ones that become objects; everything past this many is kept, together, as one mesh.",
        );
        let mut objects = f64::from(tool.plan.max_objects);
        number(app, ui, (SCOPE, "objects"), FIELD_WIDTH, OBJECTS, &mut objects);
        tool.plan.max_objects = objects.round().clamp(1.0, 10_000.0) as u32;
        ui.end_row();

        label(
            ui,
            "Show what was found",
            "Draw the shapes that were recognised over the model, as they would be built, and the \
             bodies that were not as the boxes they will be kept in.",
        );
        ui.checkbox(&mut tool.outlines, "");
        ui.end_row();
    });
}
