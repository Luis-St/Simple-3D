//! What a selection has in common, and where its panel is drawn.

use super::*;
use crate::app::App;
use crate::theme::{self};
use simple3d_core::scene::{Body, NodeId};

/// The panel's contents without the dock, so it can be drawn in either dock.
pub fn show_inside(app: &mut App, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(theme::metric::GAP, 2.0);
    // Commands requested in here run after the panel is drawn, since tree changes (a Join) would leave
    // later sections drawing removed rows.
    let mut command: Option<simple3d_core::keymap::Command> = None;
    // Opening a component switches the scene, so it waits too.
    let mut open_component = false;
    let (area, restore) = theme::list_scroll_area(ui);
    area.show(ui, |ui| {
        ui.set_style(restore);
        // Everything the panel edits, primary last, as in the selection.
        let targets: Vec<NodeId> = app.selection.iter().copied().filter(|id| app.scene.contains(*id)).collect();
        let Some(primary) = app.primary() else {
            document(app, ui);
            // Roundings shared by objects of the scene itself are held by it (issue 88).
            let root = app.scene.root();
            if !app.scene.node(root).edits.is_empty() {
                section(ui, "Scene edits", |ui| face_edits(app, ui, root));
            }
            return;
        };
        section(ui, "Object", |ui| common(app, ui, &targets));
        match shared_type(app, &targets) {
            Some(type_id) => {
                let label = simple3d_core::primitive::lookup(&type_id).map(|s| s.label).unwrap_or("");
                let note =
                    if targets.len() > 1 { format!("{} \u{00D7} {label}", targets.len()) } else { label.to_string() };
                section_titled(ui, "Dimensions", &note, |ui| primitive(app, ui, &targets, &type_id));
            }
            None => match app.scene.node(primary).body.clone() {
                Body::Group { op } if targets.len() == 1 => section(ui, "Boolean", |ui| group(app, ui, primary, op)),
                Body::Pattern { .. } if targets.len() == 1 => section(ui, "Pattern", |ui| pattern(app, ui, primary)),
                Body::Mesh { .. } if targets.len() == 1 => section(ui, "Mesh", |ui| mesh_body(app, ui, primary)),
                Body::Extrusion { .. } if targets.len() == 1 => {
                    section(ui, "Extrusion", |ui| extrusion_body(app, ui, primary))
                }
                Body::Component { .. } if targets.len() == 1 => {
                    section(ui, "Component", |ui| component_body(app, ui, primary, &mut open_component))
                }
                Body::Split { .. } if targets.len() == 1 => {
                    section(ui, "Split", |ui| split_body(app, ui, primary, &mut command));
                    // Pieces live inside the collection, so this is the only place to reach them (issue 82).
                    section(ui, "Pieces", |ui| pieces_list(app, ui, primary));
                }
                // Mixed types share no dimensions; say so rather than edit one silently.
                _ => section(ui, "Dimensions", |ui| {
                    ui.add(
                        egui::Label::new(theme::hint(
                            "The selection mixes shapes, so there is no dimension they share. Transform below still \
                             applies to all of them.",
                        ))
                        .selectable(false),
                    );
                }),
            },
        }
        if targets.len() == 1 && !app.scene.node(primary).edits.is_empty() {
            section(ui, "Edits", |ui| face_edits(app, ui, primary));
        }
        section(ui, "Transform", |ui| placement(app, ui, &targets));
        section(ui, "Measured", |ui| measurements(app, ui, primary, targets.len()));
    });
    if let Some(command) = command {
        app.run(command);
    }
    if open_component {
        if let Some(id) = app.primary() {
            app.open_component_of(id);
        }
    }
}

/// The primitive type all selected nodes share, or `None`; decides if Dimensions can cover them all.
pub(crate) fn shared_type(app: &App, targets: &[NodeId]) -> Option<String> {
    let mut found: Option<String> = None;
    for id in targets {
        let Body::Primitive { type_id, .. } = &app.scene.node(*id).body else { return None };
        match &found {
            Some(first) if first != type_id => return None,
            Some(_) => {}
            None => found = Some(type_id.clone()),
        }
    }
    found
}
