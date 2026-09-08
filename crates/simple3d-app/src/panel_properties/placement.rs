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

    // Three columns of numbers, each fronted by its axis colour: the row says
    // which axis is which without spending a character on saying so. The drag
    // is on the fields themselves, here as everywhere else in the panel.
    axis_row(app, ui, &format!("Position ({})", unit.suffix()), |app, ui, axis, name| {
        let field_id = ui.id().with((primary, "pos", axis));
        let shown =
            ui::shared_text(targets.iter().map(|t| format_length(component(app.scene.node(*t).position, axis), unit)));
        let step = unit.from_mm(app.move_snap());
        let outcome = value_field(app, ui, name, field_id, &shown, step);
        if let Some(scrubbed) = outcome.scrubbed {
            scrub_transform(app, targets, axis, unit.to_mm(scrubbed.delta), false, scrubbed.started);
        }
        if let Some(text) = outcome.committed {
            if text.trim() == ui::MIXED {
                app.fields.accept(field_id);
                return;
            }
            let mut resolved: Vec<(NodeId, f64)> = Vec::new();
            for target in targets {
                let current = component(app.scene.node(*target).position, axis);
                match ui::commit_length(&text, unit, current) {
                    Some(value) => resolved.push((*target, value)),
                    None => {
                        app.fields.reject(field_id, text.clone());
                        app.status = Status::Info(format!("\"{text}\" is not a number this field can take"));
                        return;
                    }
                }
            }
            app.fields.accept(field_id);
            app.edit("Set position", Some(&format!("pos:{primary}:{axis}")));
            for (target, value) in resolved {
                if let Some(node) = app.scene.get_mut(target) {
                    let mut p = node.position;
                    set_component(&mut p, axis, value);
                    node.position = p;
                }
            }
        }
    });

    // Read as a direction rather than as a running total: a rotation is brought
    // into [0, 360) here as well as when it is typed, because the manipulator
    // and the arrow keys turn a body too and a field that reads 725 after a
    // couple of turns of the ring is describing the gesture, not the model
    // (issue 84).
    axis_row(app, ui, "Rotation (deg)", |app, ui, axis, name| {
        let field_id = ui.id().with((primary, "rot", axis));
        let shown = ui::shared_text(
            targets.iter().map(|t| format_angle(wrap_degrees(component(app.scene.node(*t).rotation, axis)))),
        );
        let step = app.settings.rotate_snap_deg.max(1.0);
        let outcome = value_field(app, ui, name, field_id, &shown, step);
        if let Some(scrubbed) = outcome.scrubbed {
            scrub_transform(app, targets, axis, scrubbed.delta, true, scrubbed.started);
        }
        if let Some(text) = outcome.committed {
            if text.trim() == ui::MIXED {
                app.fields.accept(field_id);
                return;
            }
            let mut resolved: Vec<(NodeId, f64)> = Vec::new();
            for target in targets {
                let current = component(app.scene.node(*target).rotation, axis);
                match ui::commit_angle(&text, current) {
                    Some(value) => resolved.push((*target, value)),
                    None => {
                        app.fields.reject(field_id, text.clone());
                        app.status = Status::Info(format!("\"{text}\" is not a number this field can take"));
                        return;
                    }
                }
            }
            app.fields.accept(field_id);
            app.edit("Set rotation", Some(&format!("rot:{primary}:{axis}")));
            for (target, value) in resolved {
                if let Some(node) = app.scene.get_mut(target) {
                    let mut r = node.rotation;
                    set_component(&mut r, axis, value);
                    node.rotation = r;
                }
            }
        }
    });

    // Scale is a factor, not a measurement, so it has no unit and reads in the
    // same three-column row as the two above it. It is the one control that
    // resizes a *group*: a group has no dimensions of its own to type into.
    axis_row(app, ui, "Scale (x)", |app, ui, axis, name| {
        let field_id = ui.id().with((primary, "scale", axis));
        let shown = ui::shared_text(
            targets.iter().map(|t| format_number(component(Node::sane_scale(app.scene.node(*t).scale), axis), 4)),
        );
        let outcome = value_field(app, ui, name, field_id, &shown, 0.05);
        if let Some(scrubbed) = outcome.scrubbed {
            scrub_scale(app, targets, axis, scrubbed.delta, scrubbed.started);
        }
        if let Some(text) = outcome.committed {
            if text.trim() == ui::MIXED {
                app.fields.accept(field_id);
                return;
            }
            let mut resolved: Vec<(NodeId, f64)> = Vec::new();
            for target in targets {
                let current = component(Node::sane_scale(app.scene.node(*target).scale), axis);
                match ui::commit_factor(&text, current) {
                    Some(value) => resolved.push((*target, value)),
                    None => {
                        app.fields.reject(field_id, text.clone());
                        app.status = Status::Info(format!("\"{text}\" is not a number this field can take"));
                        return;
                    }
                }
            }
            app.fields.accept(field_id);
            app.edit("Set scale", Some(&format!("scale:{primary}:{axis}")));
            for (target, value) in resolved {
                if let Some(node) = app.scene.get_mut(target) {
                    let mut s = Node::sane_scale(node.scale);
                    set_component(&mut s, axis, value);
                    node.scale = s;
                }
            }
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
