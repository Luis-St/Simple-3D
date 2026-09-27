//! Position, rotation and scale.

use super::*;
use crate::app::{App, Status};
use crate::theme::{self};
use crate::ui::{self};
use simple3d_core::scene::{Node, NodeId};
use simple3d_core::unit::{format_angle, format_length, format_number, wrap_degrees};
use simple3d_geom::Vec3;

pub(crate) fn placement(app: &mut App, ui: &mut egui::Ui, targets: &[NodeId]) {
    let unit = app.unit();
    let Some(&primary) = targets.last() else { return };

    // Three columns, each fronted by its axis colour; dragging is on the fields themselves.
    axis_row(app, ui, &format!("Position ({})", unit.suffix()), |app, ui, axis, name| {
        let field_id = ui.id().with((primary, "pos", axis));
        let shown = ui::shared_text(targets.iter().map(|t| format_length(app.scene.node(*t).position.get(axis), unit)));
        let step = unit.from_mm(app.move_snap());
        let outcome = value_field(app, ui, name, field_id, &shown, step);
        if let Some(scrubbed) = outcome.scrubbed {
            scrub_transform(app, targets, axis, unit.to_mm(scrubbed.delta), false, scrubbed.started);
        }
        if let Some(text) = outcome.committed {
            let vector = Vector {
                label: "Set position",
                key: "pos",
                read: |node| node.position,
                write: |node, v| node.position = v,
                parse: &|text, current| ui::commit_length(text, unit, current),
            };
            commit_axis(app, targets, field_id, axis, &text, &vector);
        }
    });

    // Wrapped into [0, 360) here too, since the ring and arrow keys also turn a body and 725 would describe
    // the gesture, not the model (issue 84).
    axis_row(app, ui, "Rotation (deg)", |app, ui, axis, name| {
        let field_id = ui.id().with((primary, "rot", axis));
        let shown =
            ui::shared_text(targets.iter().map(|t| format_angle(wrap_degrees(app.scene.node(*t).rotation.get(axis)))));
        let step = app.settings.rotate_snap_deg.max(1.0);
        let outcome = value_field(app, ui, name, field_id, &shown, step);
        if let Some(scrubbed) = outcome.scrubbed {
            scrub_transform(app, targets, axis, scrubbed.delta, true, scrubbed.started);
        }
        if let Some(text) = outcome.committed {
            let vector = Vector {
                label: "Set rotation",
                key: "rot",
                read: |node| node.rotation,
                write: |node, v| node.rotation = v,
                parse: &ui::commit_angle,
            };
            commit_axis(app, targets, field_id, axis, &text, &vector);
        }
    });

    // Scale is a unitless factor, and the only way to resize a group, which has no dimensions.
    axis_row(app, ui, "Scale (x)", |app, ui, axis, name| {
        let field_id = ui.id().with((primary, "scale", axis));
        let shown = ui::shared_text(
            targets.iter().map(|t| format_number(Node::sane_scale(app.scene.node(*t).scale).get(axis), 4)),
        );
        let outcome = value_field(app, ui, name, field_id, &shown, 0.05);
        if let Some(scrubbed) = outcome.scrubbed {
            scrub_scale(app, targets, axis, scrubbed.delta, scrubbed.started);
        }
        if let Some(text) = outcome.committed {
            let vector = Vector {
                label: "Set scale",
                key: "scale",
                read: |node| Node::sane_scale(node.scale),
                write: |node, v| node.scale = v,
                parse: &ui::commit_factor,
            };
            commit_axis(app, targets, field_id, axis, &text, &vector);
        }
    });
    if targets.iter().any(|t| app.scene.node(*t).scale != Vec3::ONE) {
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(theme::hint("Scale is a factor on top of the dimensions.")).selectable(false));
            if ui.small_button("Reset to 1").clicked() {
                app.edit("Reset scale", None);
                for target in targets {
                    if let Some(node) = app.scene.get_mut(*target) {
                        node.scale = Vec3::ONE;
                    }
                }
            }
        });
    }

    step_row(app, ui);
    rotate_step_row(app, ui);
    ui.add(egui::Label::new(theme::hint("Rotations are applied X, then Y, then Z.")).selectable(false));
    if targets.len() > 1 {
        ui.add(
            egui::Label::new(theme::hint(
                "A value applies to all of them; a delta (\u{201C}+2\u{201D}, \u{201C}- 5\u{201D}) applies to each \
                 from where it already is.",
            ))
            .selectable(false),
        );
    }
}

/// A placement vector as its per-axis fields edit it.
struct Vector<'a> {
    label: &'static str,
    key: &'static str,
    read: fn(&Node) -> Vec3,
    write: fn(&mut Node, Vec3),
    parse: &'a dyn Fn(&str, f64) -> Option<f64>,
}

/// Commit `text` to one axis of `vector` on every target as one undo step, or reject it if any
/// target cannot take it. A mixed field left as it was changes nothing.
fn commit_axis(app: &mut App, targets: &[NodeId], field_id: egui::Id, axis: usize, text: &str, vector: &Vector) {
    let Some(&primary) = targets.last() else { return };
    if text.trim() == ui::MIXED {
        app.fields.accept(field_id);
        return;
    }
    let mut resolved: Vec<(NodeId, f64)> = Vec::new();
    for target in targets {
        let current = (vector.read)(app.scene.node(*target)).get(axis);
        match (vector.parse)(text, current) {
            Some(value) => resolved.push((*target, value)),
            None => {
                app.fields.reject(field_id, text.to_string());
                app.status = Status::Info(format!("\"{text}\" is not a number this field can take"));
                return;
            }
        }
    }
    app.fields.accept(field_id);
    app.edit(vector.label, Some(&format!("{}:{primary}:{axis}", vector.key)));
    for (target, value) in resolved {
        if let Some(node) = app.scene.get_mut(target) {
            let mut v = (vector.read)(node);
            v.set(axis, value);
            (vector.write)(node, v);
        }
    }
}
