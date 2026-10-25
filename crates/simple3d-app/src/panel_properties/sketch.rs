//! A sketch node's panel (issue 74): what its regions become and how far, then what the sketch is
//! made of and the button that opens it again.

use super::*;
use crate::app::App;
use crate::theme;
use simple3d_core::keymap::Command;
use simple3d_core::scene::{sketch_param_shown, NodeId, SKETCH_PARAMS};

pub(crate) fn sketch_body(app: &mut App, ui: &mut egui::Ui, id: NodeId, command: &mut Option<Command>) {
    let unit = app.unit();
    for param in &SKETCH_PARAMS {
        let shown = app.scene.node(id).params().is_some_and(|params| sketch_param_shown(param.key, params));
        if shown {
            param_field(app, ui, &[id], id, param, unit, DIMENSION_ROW);
        }
    }
    let Some(sketch) = app.scene.node(id).sketch().cloned() else { return };
    let summary = format!(
        "{} curves, {} constraints, {} regions picked",
        sketch.curves.len(),
        sketch.constraints.len(),
        sketch.regions.len()
    );
    field_row(ui, "Drawn", "What the sketch is made of.", |ui| {
        ui.add(egui::Label::new(theme::value(summary)).selectable(false));
    });
    field_row(ui, "", "", |ui| {
        let shortcut = app.keymap.shortcut_text(Command::Sketch);
        let label = if shortcut.is_empty() { "Edit sketch".to_string() } else { format!("Edit sketch ({shortcut})") };
        if ui
            .button(label)
            .on_hover_text("Open the sketch to change its curves, constraints and picked regions.")
            .clicked()
        {
            *command = Some(Command::Sketch);
        }
    });
}
