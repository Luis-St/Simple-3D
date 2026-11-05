//! The panels of a sketch node and of a feature made from one (issue 74): what the sketch is made
//! of and the button that opens it again; how far the feature sweeps and what from.

use super::*;
use crate::app::App;
use crate::theme;
use simple3d_core::keymap::Command;
use simple3d_core::scene::{feature_param_shown, feature_specs, NodeId, Profile, RevolveAxis};

pub(crate) fn sketch_body(app: &mut App, ui: &mut egui::Ui, id: NodeId, command: &mut Option<Command>) {
    let Some(sketch) = app.scene.node(id).sketch().cloned() else { return };
    let regions = app.regions_of(id, &sketch).list.len();
    let summary =
        format!("{} curves, {} constraints, {regions} closed regions", sketch.curves.len(), sketch.constraints.len());
    field_row(ui, "Drawn", "What the sketch is made of.", |ui| {
        ui.add(egui::Label::new(theme::value(summary)).selectable(false));
    });
    let made = app.scene.features_of(sketch.link).len();
    let used = match made {
        0 => "No solid yet".to_string(),
        1 => "1 solid".to_string(),
        n => format!("{n} solids"),
    };
    field_row(ui, "Made into", "The solids push / pull and revolve made from its regions.", |ui| {
        ui.add(egui::Label::new(theme::value(used)).selectable(false));
    });
    edit_button(app, ui, || *command = Some(Command::Sketch));
    ui.add(
        egui::Label::new(theme::hint(
            "Push / pull a closed region to extrude it, or revolve it about a line with the revolve tool. The \
             solids follow the sketch when it changes.",
        ))
        .selectable(false),
    );
}

/// A feature's panel: its parameters, then what it was made from, with the way back to its sketch.
pub(crate) fn feature_body(app: &mut App, ui: &mut egui::Ui, id: NodeId, command: &mut Option<Command>) {
    let Some(feature) = app.scene.node(id).feature().cloned() else { return };
    let unit = app.unit();
    for param in feature_specs(feature.is_revolve()) {
        let shown = app.scene.node(id).params().is_some_and(|params| feature_param_shown(param.key, params));
        if shown {
            param_field(app, ui, &[id], id, param, unit, DIMENSION_ROW);
        }
    }
    let sketch = feature.sketch().and_then(|sketch| app.scene.sketch_node(sketch.link));
    let from = match (&feature.profile, sketch) {
        (Profile::Face { .. }, _) => "A face of the model, as it was".to_string(),
        (Profile::Sketch { .. }, Some(sketch)) => format!("A region of {}", app.scene.node(sketch).name),
        (Profile::Sketch { .. }, None) => "A region of a deleted sketch".to_string(),
    };
    field_row(ui, "Profile", "What is swept.", |ui| {
        ui.add(egui::Label::new(theme::value(from)).selectable(false));
    });
    if let Some(axis) = &feature.axis {
        let about = match axis {
            RevolveAxis::Line(_) => "A line of the sketch",
            RevolveAxis::Fixed(..) => "An edge or axis, as it was",
        };
        field_row(ui, "Axis", "The line it turns about.", |ui| {
            ui.add(egui::Label::new(theme::value(about)).selectable(false));
        });
    }
    if let Some(sketch) = sketch {
        let mut open = false;
        edit_button(app, ui, || open = true);
        if open {
            app.select_only(sketch);
            *command = Some(Command::Sketch);
        }
    }
}

/// The button that opens the sketch, with its key.
fn edit_button(app: &App, ui: &mut egui::Ui, mut clicked: impl FnMut()) {
    field_row(ui, "", "", |ui| {
        let shortcut = app.keymap.shortcut_text(Command::Sketch);
        let label = if shortcut.is_empty() { "Edit sketch".to_string() } else { format!("Edit sketch ({shortcut})") };
        if ui.button(label).on_hover_text("Open the sketch to change its curves and constraints.").clicked() {
            clicked();
        }
    });
}
