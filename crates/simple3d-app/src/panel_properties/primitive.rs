//! A primitive's own parameters, derived from its declaration.

use super::*;
use crate::app::{App, Status};
use crate::theme::{self, token};
use crate::ui::{self, Commit};
use simple3d_core::primitive::{ParamKind, ParamValue};
use simple3d_core::scene::NodeId;
use simple3d_core::unit::Unit;

/// What one node holds for a parameter.
pub(crate) fn param_value(app: &App, id: NodeId, key: &str, default: ParamValue) -> ParamValue {
    app.scene.node(id).params().and_then(|p| p.get(key).copied()).unwrap_or(default)
}

pub(crate) fn primitive(app: &mut App, ui: &mut egui::Ui, targets: &[NodeId], type_id: &str) {
    let Some(spec) = simple3d_core::primitive::lookup(type_id) else {
        ui.colored_label(token::DANGER, format!("Unknown primitive type \"{type_id}\""));
        return;
    };
    let Some(&id) = targets.last() else { return };
    let unit = app.unit();
    let params = app.scene.node(id).params().cloned().unwrap_or_default();

    // The axes each dimension measures along, from the resize drivers (issue 110), asked of the type's
    // defaults so a swept shape's chips do not change.
    let drivers = (spec.axes)(&spec.default_params());
    let along = |key: &str| -> Vec<usize> {
        (0..3).filter(|&axis| drivers[axis].is_some_and(|driver| driver.param == key)).collect()
    };
    // A type with no axis to show gets no chip column.
    let chips = drivers.iter().any(Option::is_some);
    for param in spec.params {
        if !spec.param_visible(param, &params) {
            continue;
        }
        let axes = along(param.key);
        let axes = chips.then_some(axes.as_slice());
        param_field_on(app, ui, targets, id, param, param.label, unit, DIMENSION_ROW, axes);
    }

    if spec.segmented {
        field_row(ui, "Segments", "Overrides the scene default for this object's curved surfaces.", |ui| {
            let mut overridden = app.scene.node(id).segments.is_some();
            if ui.checkbox(&mut overridden, "").changed() {
                app.edit("Segment override", None);
                let default = app.scene.settings.default_segments;
                for target in targets {
                    if let Some(node) = app.scene.get_mut(*target) {
                        node.segments = if overridden { Some(default) } else { None };
                    }
                }
            }
            let default = app.scene.settings.default_segments;
            match app.scene.node(id).segments {
                Some(current) => {
                    let field_id = ui.id().with((id, "segments"));
                    let width = room_left(ui).max(40.0);
                    ui.scope(|ui| {
                        ui.set_width(width);
                        let field = Scalar {
                            grip: "Object segments",
                            id: field_id,
                            kind: SEGMENTS,
                            current: current as f64,
                            step: 1.0,
                        };
                        scalar_field(app, ui, field, |app, count, started| {
                            edit_or_touch(app, started, "Segments", &format!("segments:{id}"));
                            for target in targets {
                                if let Some(node) = app.scene.get_mut(*target) {
                                    node.segments = Some(count as u32);
                                }
                            }
                        });
                    });
                }
                None => {
                    ui.add(egui::Label::new(theme::hint(format!("{default} (scene default)"))).selectable(false));
                }
            }
        });
    }
}

/// Apply one typed value to every selected node; a delta (`+2`) resolves per node. A value none
/// can read marks the field and changes nothing.
pub(crate) fn set_shared_param(
    app: &mut App,
    targets: &[NodeId],
    param: &simple3d_core::primitive::ParamSpec,
    kind: ParamKind,
    unit: Unit,
    field_id: egui::Id,
    text: String,
) {
    // The em dash means disagreement; leaving it must write nothing.
    if text.trim() == ui::MIXED {
        app.fields.accept(field_id);
        return;
    }
    let mut resolved: Vec<(NodeId, ParamValue)> = Vec::new();
    for target in targets {
        let current = ui::param_number(param_value(app, *target, param.key, param.default));
        match ui::commit_param(&text, kind, unit, current) {
            Commit::Value(value) => resolved.push((*target, value)),
            Commit::Revert => {
                app.fields.reject(field_id, text.clone());
                app.status = Status::Info(format!("\"{text}\" is not a number this field can take"));
                return;
            }
        }
    }
    app.fields.accept(field_id);
    let coalesce = format!("param:{:?}:{}", targets.last(), param.key);
    app.edit(&format!("Set {}", param.label), Some(&coalesce));
    for (target, value) in resolved {
        set_param(app, target, param.key, value);
        apply_lock(app, target, param.lock_group, param.key, value);
    }
}
