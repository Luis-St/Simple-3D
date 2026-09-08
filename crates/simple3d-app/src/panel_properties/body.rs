//! Stored meshes and the collections a split produced.

use super::*;
use crate::app::App;
use crate::theme::{self};
use simple3d_core::scene::NodeId;

/// A stored mesh's panel. There are no parameters -- that is what being a mesh
/// means -- so what it can say is how much geometry there is and what can still
/// be done to it.
pub(crate) fn mesh_body(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let Some(mesh) = app.scene.node(id).mesh() else { return };
    let triangles = mesh.triangle_count();
    let vertices = mesh.mesh.positions.len();
    field_row(ui, "Triangles", "", |ui| {
        ui.add(egui::Label::new(theme::value(triangles.to_string())).selectable(false));
    });
    field_row(ui, "Vertices", "", |ui| {
        ui.add(egui::Label::new(theme::value(vertices.to_string())).selectable(false));
    });
    ui.add(
        egui::Label::new(theme::hint(
            "Geometry with no parameters behind it. It can still be moved, painted, cut with a boolean and \
             split into smaller pieces.",
        ))
        .selectable(false),
    );
}

/// A split's panel (issue 82): what it was made from, how many pieces it is in,
/// and the one button that undoes the break.
///
/// The point of the section is that a break is not a one-way door. The object
/// the pieces came from is still here, whole, with its parameters -- so the
/// panel names it rather than leaving the user to remember what a group of
/// nameless meshes used to be.
pub(crate) fn split_body(
    app: &mut App,
    ui: &mut egui::Ui,
    id: NodeId,
    command: &mut Option<simple3d_core::keymap::Command>,
) {
    let Some(original) = app.scene.node(id).split_original().cloned() else { return };
    let pieces = app.scene.node(id).children.len();
    // A shape is named by its label -- "Rounded box", not "rounded_box" -- and
    // every other body is its own type name, which is already the word for it.
    let kind = simple3d_core::primitive::lookup(&original.type_id).map_or(original.type_id.as_str(), |s| s.label);
    field_row(ui, "Pieces", "Each piece is an object of its own: move it, paint it, export it apart.", |ui| {
        ui.add(egui::Label::new(theme::value(pieces.to_string())).selectable(false));
    });
    field_row(ui, "Made from", "The object this was broken apart from, kept whole so it can come back.", |ui| {
        ui.add(egui::Label::new(theme::value(format!("{} ({kind})", original.name))).selectable(false));
    });
    // How the pieces were cut, for a split that was cut into a pattern rather
    // than separated into the pieces it was already in (issue 82). A split has
    // no parameters of its own to edit -- the pieces are geometry -- so this is
    // a statement of what was done, and the button below is how it is changed.
    if let Some(plan) = app.scene.node(id).split_plan().cloned() {
        // One row per cut, numbered when there is more than one: two cuts are
        // two patterns, and a single line naming both reads as one pattern
        // nobody can find the numbers of.
        for (index, tiling) in plan.passes.iter().enumerate() {
            let label = if plan.passes.len() > 1 { format!("Cut {}", index + 1) } else { "Cut into".to_string() };
            field_row(ui, &label, "The pattern of cells the pieces were cut out by.", |ui| {
                ui.add(egui::Label::new(theme::value(describe_tiling(tiling, app.unit()))).selectable(false));
            });
        }
    }
    field_row(ui, "", "", |ui| {
        let shortcut = app.keymap.shortcut_text(simple3d_core::keymap::Command::Rejoin);
        let label = if shortcut.is_empty() {
            "Join back together".to_string()
        } else {
            format!("Join back together ({shortcut})")
        };
        if ui.button(label).clicked() {
            *command = Some(simple3d_core::keymap::Command::Rejoin);
        }
        // Cutting a split again replaces its pieces and keeps everything else,
        // so changing the pattern is one gesture rather than a join, a re-split
        // and a rename.
        if ui
            .button("Split differently\u{2026}")
            .on_hover_text("Cut the same object into a different pattern of pieces. What it was made from is kept.")
            .clicked()
        {
            *command = Some(simple3d_core::keymap::Command::SplitIntoPieces);
        }
    });
}

pub(crate) fn describe_tiling(tiling: &simple3d_geom::tiling::Tiling, unit: simple3d_core::unit::Unit) -> String {
    // With the unit on the numbers, unlike a field: this is a sentence about
    // what was done, and "in layers of 4" reads as four layers.
    let length = |mm: f64| format!("{} {}", simple3d_core::unit::format_length(mm, unit), unit.suffix());
    let mut text = match tiling.kind.has_depth() {
        true => format!(
            "{} of {} x {}",
            tiling.kind.label(),
            simple3d_core::unit::format_length(tiling.size, unit),
            length(tiling.depth)
        ),
        false => format!("{} of {}", tiling.kind.label(), length(tiling.size)),
    };
    text.push_str(&format!(" through {}", ["X", "Y", "Z"][(tiling.axis as usize).min(2)]));
    if tiling.angle != 0.0 {
        text.push_str(&format!(", turned {}\u{b0}", simple3d_core::unit::format_angle(tiling.angle)));
    }
    if tiling.layer > 0.0 {
        text.push_str(&format!(", in layers of {}", length(tiling.layer)));
    }
    text
}
