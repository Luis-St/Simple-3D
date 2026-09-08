//! What a selection has in common, and where its panel is drawn.

use super::*;
use crate::app::App;
use crate::theme::{self};
use simple3d_core::scene::{Body, NodeId};

/// The panel's contents, without the dock around them, so the same panel can be
/// drawn in either dock.
pub fn show_inside(app: &mut App, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(theme::metric::GAP, 2.0);
    // A command a button in here asks for is run *after* the panel is drawn,
    // never on the spot: the sections below this one are still to be laid out
    // from the selection this frame started with, and a command that changes
    // the tree -- joining a split back together takes a node out of it -- would
    // leave them drawing a row that is no longer there.
    let mut command: Option<simple3d_core::keymap::Command> = None;
    let (area, restore) = theme::list_scroll_area(ui);
    area.show(ui, |ui| {
        ui.set_style(restore);
        // Everything the panel edits, primary last -- the same order the
        // selection itself is in, so "the one being edited" is unambiguous.
        let targets: Vec<NodeId> = app.selection.iter().copied().filter(|id| app.scene.contains(*id)).collect();
        let Some(primary) = app.primary() else {
            document(app, ui);
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
                Body::Split { .. } if targets.len() == 1 => {
                    section(ui, "Split", |ui| split_body(app, ui, primary, &mut command));
                    // The pieces are inside the collection rather than in the
                    // tree, so this is the only place they can be got at
                    // (issue 82).
                    section(ui, "Pieces", |ui| pieces_list(app, ui, primary));
                }
                // A selection of different types has no shared dimension to
                // offer. Saying so beats an empty panel or a set of fields that
                // would edit only one of them without saying which.
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
        section(ui, "Transform", |ui| placement(app, ui, &targets));
        section(ui, "Measured", |ui| measurements(app, ui, primary, targets.len()));
    });
    if let Some(command) = command {
        app.run(command);
    }
}

/// The primitive type every selected node has, or `None` when they are not all
/// the same kind of thing. This is what decides whether a Dimensions panel can
/// speak for the whole selection.
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
