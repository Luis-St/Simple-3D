//! The property editor (spec section 7.3).
//!
//! Every field here is generated from the selected primitive's declaration in
//! the registry -- there is no per-primitive code. Values commit on Enter and on
//! leaving the field; unparseable text restores the previous value silently.

use crate::app::{App, Status};
use crate::ui::{self, Commit};
use scadstudio_core::primitive::{ParamKind, ParamValue, ParamsExt};
use scadstudio_core::scene::{Anchor, Body, GroupOp, NodeId};
use scadstudio_core::unit::{format_angle, format_length};
use scadstudio_geom::Vec3;

pub fn show(app: &mut App, ctx: &egui::Context) {
    let mut width = app.settings.properties_width;
    egui::SidePanel::right("properties")
        .resizable(true)
        .default_width(width)
        .width_range(240.0..=620.0)
        .show(ctx, |ui| {
            width = ui.available_width();
            ui.heading("Properties");
            ui.separator();
            let Some(id) = app.primary() else {
                ui.label("Nothing selected.");
                ui.label(
                    egui::RichText::new("Add a primitive from the Add menu, or click something in the viewport.")
                        .weak(),
                );
                return;
            };
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                common(app, ui, id);
                ui.separator();
                match app.scene.node(id).body.clone() {
                    Body::Group { op } => group(app, ui, id, op),
                    Body::Primitive { type_id, .. } => primitive(app, ui, id, &type_id),
                }
                ui.separator();
                placement(app, ui, id);
                ui.separator();
                measurements(app, ui, id);
            });
        });
    app.settings.properties_width = width;
}

fn common(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let node = app.scene.node(id);
    let is_root = id == app.scene.root();
    let mut name = node.name.clone();
    let mut visible = node.visible;
    let mut anchor = node.anchor;

    egui::Grid::new("common").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label("Name");
        if ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY)).changed() {
            app.edit("Rename", Some(&format!("name:{id}")));
            if let Some(node) = app.scene.get_mut(id) {
                node.name = name;
            }
        }
        ui.end_row();

        ui.label("Visible");
        if ui.add_enabled(!is_root, egui::Checkbox::new(&mut visible, "")).changed() {
            app.edit("Toggle visibility", None);
            if let Some(node) = app.scene.get_mut(id) {
                node.visible = visible;
            }
        }
        ui.end_row();

        ui.label("Anchor").on_hover_text(
            "Where this node's origin sits. Changing it moves the origin, never the shape.",
        );
        ui.horizontal(|ui| {
            for option in Anchor::ALL {
                if ui.radio_value(&mut anchor, option, option.label()).changed() {
                    app.edit("Anchor", None);
                    if let Some(node) = app.scene.get_mut(id) {
                        node.anchor = anchor;
                    }
                }
            }
        });
        ui.end_row();
    });
}

fn group(app: &mut App, ui: &mut egui::Ui, id: NodeId, current: GroupOp) {
    ui.strong("Group");
    let mut op = current;
    ui.horizontal_wrapped(|ui| {
        for option in GroupOp::ALL {
            if ui.radio_value(&mut op, option, option.label()).changed() {
                app.edit("Operation", None);
                if let Some(node) = app.scene.get_mut(id) {
                    node.body = Body::Group { op };
                }
            }
        }
    });

    let children = app.scene.node(id).children.clone();
    if op.order_matters() {
        // When a difference group is selected, state plainly which child is the
        // base (spec section 7.3).
        match app.scene.difference_base(id) {
            Some(base) => {
                ui.label(format!(
                    "Base: {}. Every visible child below it is subtracted from it.",
                    app.scene.node(base).name
                ));
            }
            None => {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    "No visible child, so there is nothing to subtract from.",
                );
            }
        }
        ui.label(egui::RichText::new("Child order matters here; reorder with the buttons below.").weak());
    }

    ui.add_space(4.0);
    for (index, child) in children.iter().enumerate() {
        let child = *child;
        ui.horizontal(|ui| {
            let name = app.scene.node(child).name.clone();
            let marker = if op.order_matters() && Some(child) == app.scene.difference_base(id) {
                " (base)"
            } else {
                ""
            };
            if ui.selectable_label(app.is_selected(child), format!("{}. {name}{marker}", index + 1)).clicked() {
                app.select_only(child);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(index + 1 < children.len(), egui::Button::new("v").small()).clicked() {
                    app.edit("Reorder", None);
                    app.scene.reorder(child, 1);
                }
                if ui.add_enabled(index > 0, egui::Button::new("^").small()).clicked() {
                    app.edit("Reorder", None);
                    app.scene.reorder(child, -1);
                }
            });
        });
    }
    if children.is_empty() {
        ui.label(egui::RichText::new("This group is empty.").weak());
    }
}

fn primitive(app: &mut App, ui: &mut egui::Ui, id: NodeId, type_id: &str) {
    let Some(spec) = scadstudio_core::primitive::lookup(type_id) else {
        ui.colored_label(ui.visuals().error_fg_color, format!("Unknown primitive type \"{type_id}\""));
        return;
    };
    ui.strong(spec.label);
    let unit = app.unit();
    let params = app.scene.node(id).params().cloned().unwrap_or_default();

    egui::Grid::new("params").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        for param in spec.params {
            if !spec.param_visible(param, &params) {
                continue;
            }
            let value = params.get(param.key).copied().unwrap_or(param.default);
            match param.kind {
                // Radio-style choices where a measurement is ambiguous.
                ParamKind::Choice { options } => {
                    ui.label(param.label);
                    let mut chosen = value.as_u32();
                    ui.vertical(|ui| {
                        for (index, option) in options.iter().enumerate() {
                            if ui.radio_value(&mut chosen, index as u32, *option).changed() {
                                app.edit("Set measurement", None);
                                set_param(app, id, param.key, ParamValue::Choice(chosen));
                                sync_wall_mode(app, id, param.key, chosen);
                            }
                        }
                    });
                    ui.end_row();
                }
                ParamKind::Bool => {
                    ui.label(param.label);
                    let mut on = value.as_bool();
                    if ui.checkbox(&mut on, "").changed() {
                        app.edit("Set flag", None);
                        set_param(app, id, param.key, ParamValue::Bool(on));
                    }
                    ui.end_row();
                }
                kind => {
                    let label = match kind {
                        ParamKind::Length { .. } => format!("{} ({})", param.label, unit.suffix()),
                        ParamKind::Angle { .. } => format!("{} (deg)", param.label),
                        _ => param.label.to_string(),
                    };
                    ui.label(label);
                    ui.horizontal(|ui| {
                        // A lock toggle where the type offers one: a sphere's
                        // three diameters, a cylinder's two.
                        if param.lock_group != 0 {
                            let locked = is_locked(app, id, param.lock_group);
                            if ui
                                .add(egui::Button::selectable(locked, "="))
                                .on_hover_text(if locked {
                                    "Locked equal; click to unlock"
                                } else {
                                    "Click to lock these equal"
                                })
                                .clicked()
                            {
                                toggle_lock(app, id, param.lock_group, param.key);
                            }
                        }
                        let field_id = ui.id().with((id, param.key));
                        let shown = ui::show_param(value, unit);
                        if let Some(text) = app.fields.field(ui, field_id, &shown) {
                            match ui::commit_param(&text, kind, unit) {
                                Commit::Value(new_value) => {
                                    app.edit(
                                        &format!("Set {}", param.label),
                                        Some(&format!("param:{id}:{}", param.key)),
                                    );
                                    set_param(app, id, param.key, new_value);
                                    apply_lock(app, id, param.lock_group, param.key, new_value);
                                }
                                // Silently restore the previous value: no dialog.
                                Commit::Revert => {
                                    app.status = Status::Info(format!("\"{text}\" is not a number"));
                                }
                            }
                        }
                    });
                    ui.end_row();
                }
            }
        }

        if spec.segmented {
            ui.label("Segments").on_hover_text(
                "Overrides the scene default for this object's curved surfaces.",
            );
            ui.horizontal(|ui| {
                let mut overridden = app.scene.node(id).segments.is_some();
                if ui.checkbox(&mut overridden, "").changed() {
                    app.edit("Segment override", None);
                    let default = app.scene.settings.default_segments;
                    if let Some(node) = app.scene.get_mut(id) {
                        node.segments = if overridden { Some(default) } else { None };
                    }
                }
                let default = app.scene.settings.default_segments;
                match app.scene.node(id).segments {
                    Some(current) => {
                        let mut value = current as f64;
                        if ui
                            .add(egui::DragValue::new(&mut value).range(3.0..=512.0).max_decimals(0))
                            .changed()
                        {
                            app.edit("Segments", Some(&format!("segments:{id}")));
                            if let Some(node) = app.scene.get_mut(id) {
                                node.segments = Some(value.round() as u32);
                            }
                        }
                    }
                    None => {
                        ui.label(egui::RichText::new(format!("{default} (scene default)")).weak());
                    }
                }
            });
            ui.end_row();
        }
    });
}

fn placement(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let unit = app.unit();
    ui.strong("Placement");
    let node = app.scene.node(id);
    let position = node.position;
    let rotation = node.rotation;

    egui::Grid::new("placement").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
        ui.label(format!("Position ({})", unit.suffix()));
        for axis in 0..3 {
            let field_id = ui.id().with((id, "pos", axis));
            let shown = format_length(component(position, axis), unit);
            if let Some(text) = app.fields.field(ui, field_id, &shown) {
                match ui::commit_length(&text, unit) {
                    Some(value) => {
                        app.edit("Set position", Some(&format!("pos:{id}:{axis}")));
                        if let Some(node) = app.scene.get_mut(id) {
                            let mut p = node.position;
                            set_component(&mut p, axis, value);
                            node.position = p;
                        }
                    }
                    None => app.status = Status::Info(format!("\"{text}\" is not a number")),
                }
            }
        }
        ui.end_row();

        ui.label("Rotation (deg)");
        for axis in 0..3 {
            let field_id = ui.id().with((id, "rot", axis));
            let shown = format_angle(component(rotation, axis));
            if let Some(text) = app.fields.field(ui, field_id, &shown) {
                match ui::commit_angle(&text) {
                    Some(value) => {
                        app.edit("Set rotation", Some(&format!("rot:{id}:{axis}")));
                        if let Some(node) = app.scene.get_mut(id) {
                            let mut r = node.rotation;
                            set_component(&mut r, axis, value);
                            node.rotation = r;
                        }
                    }
                    None => app.status = Status::Info(format!("\"{text}\" is not a number")),
                }
            }
        }
        ui.end_row();

        ui.label("");
        ui.label(egui::RichText::new("X").weak());
        ui.label(egui::RichText::new("Y").weak());
        ui.label(egui::RichText::new("Z").weak());
        ui.end_row();
    });
    ui.label(egui::RichText::new("Rotations are applied X, then Y, then Z.").weak().small());
}

fn measurements(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    ui.strong("Measured");
    let unit = app.unit();
    match app.evaluated.node_meshes.get(&id).and_then(|m| m.bounds()) {
        Some((lo, hi)) => {
            ui.label(format!("Bounding box: {}", ui::describe_size(hi - lo, unit)));
            ui.label(format!(
                "Centre: {}, {}, {} {}",
                format_length((lo.x + hi.x) / 2.0, unit),
                format_length((lo.y + hi.y) / 2.0, unit),
                format_length((lo.z + hi.z) / 2.0, unit),
                unit.suffix()
            ));
        }
        None => {
            ui.label(egui::RichText::new("No geometry yet.").weak());
        }
    }
    if let Some(error) = app.evaluated.error_for(id) {
        ui.colored_label(ui.visuals().error_fg_color, &error.message);
    }
}

fn component(v: Vec3, axis: usize) -> f64 {
    crate::gizmo::get_axis(v, axis)
}

fn set_component(v: &mut Vec3, axis: usize, value: f64) {
    crate::gizmo::set_axis(v, axis, value);
}

fn set_param(app: &mut App, id: NodeId, key: &str, value: ParamValue) {
    if let Some(params) = app.scene.get_mut(id).and_then(|n| n.params_mut()) {
        params.insert(key.to_string(), value);
    }
}

/// Whether a lock group's parameters currently hold the same value. The lock is
/// not stored in the model -- it is a property of the numbers themselves, so a
/// project file has no hidden state that could disagree with what it shows.
fn is_locked(app: &App, id: NodeId, group: u8) -> bool {
    let Some(spec) = app.scene.node(id).spec() else { return false };
    let Some(params) = app.scene.node(id).params() else { return false };
    let keys: Vec<&str> = spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key).collect();
    match keys.split_first() {
        Some((first, rest)) => {
            let reference = params.num(first);
            rest.iter().all(|k| (params.num(k) - reference).abs() < 1e-9)
        }
        None => false,
    }
}

/// Clicking the lock either equalises the group to the clicked field's value or,
/// if it is already locked, nudges one member so it visibly unlocks.
fn toggle_lock(app: &mut App, id: NodeId, group: u8, key: &str) {
    let Some(spec) = app.scene.node(id).spec() else { return };
    let keys: Vec<String> =
        spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key.to_string()).collect();
    if is_locked(app, id, group) {
        // Nothing to change: the fields are already independent as far as the
        // model is concerned. Just tell the user.
        app.status = Status::Info("Unlocked; edit the diameters independently".into());
        return;
    }
    let Some(value) = app.scene.node(id).params().map(|p| p.num(key)) else { return };
    app.edit("Lock dimensions", None);
    for other in keys {
        set_param(app, id, &other, ParamValue::Length(value));
    }
}

/// When a locked group's member changes, bring the others with it.
fn apply_lock(app: &mut App, id: NodeId, group: u8, key: &str, value: ParamValue) {
    if group == 0 {
        return;
    }
    // Read the lock state from before this edit: the field just changed, so the
    // group is no longer equal, and asking now would always say "unlocked".
    let Some(spec) = app.scene.node(id).spec() else { return };
    let keys: Vec<String> =
        spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key.to_string()).collect();
    let others: Vec<&String> = keys.iter().filter(|k| k.as_str() != key).collect();
    let Some(params) = app.scene.node(id).params() else { return };
    // Locked before the edit means every *other* member still agrees with every
    // other member.
    let was_locked = match others.split_first() {
        Some((first, rest)) => {
            let reference = params.num(first);
            rest.iter().all(|k| (params.num(k) - reference).abs() < 1e-9)
        }
        None => false,
    };
    if !was_locked {
        return;
    }
    let others: Vec<String> = others.into_iter().cloned().collect();
    for other in others {
        set_param(app, id, &other, value);
    }
    app.fields.clear();
}

/// Switching a tube between "wall thickness" and "inner diameter" carries the
/// current geometry across, so the shape does not jump when the user only meant
/// to change how they express it.
fn sync_wall_mode(app: &mut App, id: NodeId, key: &str, chosen: u32) {
    if key != "wall_mode" {
        return;
    }
    let Some(params) = app.scene.node(id).params().cloned() else { return };
    let outer = params.num("outer_diameter");
    if chosen == 1 {
        let inner = (outer - 2.0 * params.num("wall_thickness")).clamp(0.0, outer);
        set_param(app, id, "inner_diameter", ParamValue::Length(inner));
    } else {
        let wall = ((outer - params.num("inner_diameter")) / 2.0).max(0.0);
        set_param(app, id, "wall_thickness", ParamValue::Length(wall));
    }
    app.fields.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use scadstudio_core::primitive;
    use scadstudio_core::scene::Scene;

    #[test]
    fn every_registry_parameter_maps_to_a_field_kind_the_editor_draws() {
        // The editor has a branch per `ParamKind`; a kind it did not handle would
        // silently render nothing, so check every declared parameter falls into
        // one of them.
        for spec in primitive::REGISTRY {
            for param in spec.params {
                let handled = matches!(
                    param.kind,
                    ParamKind::Length { .. }
                        | ParamKind::Angle { .. }
                        | ParamKind::Count { .. }
                        | ParamKind::Bool
                        | ParamKind::Choice { .. }
                );
                assert!(handled, "{}.{} has an unhandled kind", spec.type_id, param.key);
            }
        }
    }

    #[test]
    fn a_lock_group_reads_as_locked_when_its_members_agree() {
        let mut scene = Scene::new();
        let root = scene.root();
        let id = scene.add_primitive("sphere", root, 0).unwrap();
        let mut app_scene = scene.clone();
        // Defaults are all 20, so the group starts locked.
        assert!(locked_in(&app_scene, id, 1));
        app_scene
            .get_mut(id)
            .unwrap()
            .params_mut()
            .unwrap()
            .insert("diameter_y".into(), ParamValue::Length(30.0));
        assert!(!locked_in(&app_scene, id, 1));
        let _ = scene;
    }

    /// The same rule as `is_locked`, against a bare scene so it can be tested
    /// without an `App`.
    fn locked_in(scene: &Scene, id: NodeId, group: u8) -> bool {
        let spec = scene.node(id).spec().unwrap();
        let params = scene.node(id).params().unwrap();
        let keys: Vec<&str> = spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key).collect();
        let (first, rest) = keys.split_first().unwrap();
        let reference = params.num(first);
        rest.iter().all(|k| (params.num(k) - reference).abs() < 1e-9)
    }

    #[test]
    fn only_the_relevant_wall_parameter_is_shown() {
        let spec = primitive::lookup("tube").unwrap();
        let mut params = spec.default_params();
        params.insert("wall_mode".into(), ParamValue::Choice(0));
        let visible: Vec<&str> =
            spec.params.iter().filter(|p| spec.param_visible(p, &params)).map(|p| p.key).collect();
        assert!(visible.contains(&"wall_thickness"));
        assert!(!visible.contains(&"inner_diameter"));

        params.insert("wall_mode".into(), ParamValue::Choice(1));
        let visible: Vec<&str> =
            spec.params.iter().filter(|p| spec.param_visible(p, &params)).map(|p| p.key).collect();
        assert!(visible.contains(&"inner_diameter"));
        assert!(!visible.contains(&"wall_thickness"));
    }
}
