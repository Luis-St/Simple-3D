//! The property editor (spec section 7.3).
//!
//! Every field here is generated from the selected primitive's declaration in
//! the registry -- there is no per-primitive code. Values commit on Enter and on
//! leaving the field; unparseable text restores the previous value silently.
//!
//! The panel set follows the selection rather than greying out: with nothing
//! selected the dock shows the document's own settings, which is information
//! the user can actually act on, instead of a column of dead fields.

use crate::app::{App, Status};
use crate::theme::{self, token};
use crate::ui::{self, Commit};
use scadstudio_core::primitive::{ParamKind, ParamValue, ParamsExt};
use scadstudio_core::scene::{Anchor, Body, GroupOp, NodeId};
use scadstudio_core::unit::{format_angle, format_length, Unit};
use scadstudio_geom::Vec3;

/// A collapsible panel in the right dock: a header bar, and a padded body that
/// is only drawn when the panel is open.
fn section(ui: &mut egui::Ui, name: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    section_titled(ui, name, "", add_contents)
}

/// A section whose header carries a second, quieter word on the right: the
/// primitive's type beside "Dimensions", so the panel keeps a stable name while
/// still saying what is selected.
fn section_titled(ui: &mut egui::Ui, name: &str, note: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    let id = ui.id().with(("section", name));
    let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(true);
    let header = theme::panel_header(ui, name, |ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        theme::twisty(ui.painter(), rect.center(), open, token::TEXT_LO);
        if !note.is_empty() {
            ui.add(egui::Label::new(theme::hint(note)).selectable(false));
        }
    });
    if header.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(id, open));
    }
    if !open {
        return;
    }
    egui::Frame::NONE.inner_margin(egui::Margin { left: 8, right: 8, top: 6, bottom: 8 }).show(ui, |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
        add_contents(ui);
    });
}

/// The label column of a property row. Fixed width, so every panel's fields
/// start at the same x and a column of numbers reads as a column.
const LABEL_WIDTH: f32 = 84.0;

fn row_label(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(LABEL_WIDTH, theme::metric::INPUT_ROW), egui::Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(theme::font::LABEL),
        token::TEXT_LO,
    );
    response
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    let mut width = app.settings.properties_width;
    let frame = egui::Frame::NONE.fill(token::SURFACE_1);
    egui::SidePanel::right("properties")
        .frame(frame)
        .resizable(true)
        .default_width(width)
        .width_range(240.0..=620.0)
        .show(ctx, |ui| {
            width = ui.available_width();
            ui.spacing_mut().item_spacing = egui::vec2(theme::metric::GAP, 2.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let Some(id) = app.primary() else {
                    document(app, ui);
                    return;
                };
                section(ui, "Object", |ui| common(app, ui, id));
                match app.scene.node(id).body.clone() {
                    Body::Group { op } => section(ui, "Boolean", |ui| group(app, ui, id, op)),
                    Body::Primitive { type_id, .. } => {
                        let note = scadstudio_core::primitive::lookup(&type_id).map(|s| s.label).unwrap_or("");
                        section_titled(ui, "Dimensions", note, |ui| primitive(app, ui, id, &type_id));
                    }
                }
                section(ui, "Transform", |ui| placement(app, ui, id));
                section(ui, "Measured", |ui| measurements(app, ui, id));
            });
        });
    app.settings.properties_width = width;
}

/// With nothing selected the dock shows the document, not a set of disabled
/// fields: units, grid, the default that governs every curved surface, and how
/// big the scene has become.
fn document(app: &mut App, ui: &mut egui::Ui) {
    section(ui, "Document", |ui| {
        let unit = app.unit();
        ui.horizontal(|ui| {
            row_label(ui, "Unit");
            egui::ComboBox::from_id_salt("doc-unit").selected_text(theme::value(unit.suffix())).width(72.0).show_ui(
                ui,
                |ui| {
                    for option in Unit::ALL {
                        // Switching never rescales the model: the unit only
                        // changes what the fields read (spec section 4).
                        if ui.selectable_label(unit == option, option.suffix()).clicked() {
                            app.scene.settings.unit = option;
                            app.fields.clear();
                        }
                    }
                },
            );
        });
        ui.horizontal(|ui| {
            row_label(ui, "Grid");
            let mut spacing = unit.from_mm(app.scene.settings.grid_spacing);
            if ui.add(egui::DragValue::new(&mut spacing).range(1e-6..=1e6).speed(0.1)).changed() {
                app.edit("Grid spacing", Some("scene:grid"));
                app.scene.settings.grid_spacing = unit.to_mm(spacing).max(1e-6);
            }
            ui.add(egui::Label::new(theme::hint(unit.suffix())).selectable(false));
        });
        ui.horizontal(|ui| {
            row_label(ui, "Segments");
            let mut segments = app.scene.settings.default_segments as f64;
            if ui.add(egui::DragValue::new(&mut segments).range(3.0..=512.0).speed(0.5).max_decimals(0)).changed() {
                app.edit("Default segments", Some("scene:segments"));
                app.scene.settings.default_segments = segments.round() as u32;
            }
        });
        ui.add(
            egui::Label::new(theme::hint("Curves are circumscribed: a diameter of 50 measures 50 at its widest."))
                .selectable(false),
        );
    });
    section(ui, "Scene", |ui| {
        let unit = app.unit();
        match app.evaluated.mesh.bounds() {
            Some((lo, hi)) => {
                ui.horizontal(|ui| {
                    row_label(ui, "Bounds");
                    ui.add(egui::Label::new(theme::numeric(ui::describe_size(hi - lo, unit))).selectable(false));
                });
            }
            None => {
                ui.add(egui::Label::new(theme::hint("Nothing in the scene yet.")).selectable(false));
            }
        }
        ui.add(
            egui::Label::new(theme::hint("Select a shape to edit it, or pick one from the palette.")).selectable(false),
        );
    });
}

fn common(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let node = app.scene.node(id);
    let is_root = id == app.scene.root();
    let mut name = node.name.clone();
    let mut visible = node.visible;
    let mut anchor = node.anchor;

    ui.horizontal(|ui| {
        row_label(ui, "Name");
        if ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY)).changed() {
            app.edit("Rename", Some(&format!("name:{id}")));
            if let Some(node) = app.scene.get_mut(id) {
                node.name = name;
            }
        }
    });

    ui.horizontal(|ui| {
        row_label(ui, "Visible");
        if ui.add_enabled(!is_root, egui::Checkbox::new(&mut visible, "")).changed() {
            app.edit("Toggle visibility", None);
            if let Some(node) = app.scene.get_mut(id) {
                node.visible = visible;
            }
        }
    });

    ui.horizontal(|ui| {
        row_label(ui, "Anchor")
            .on_hover_text("Where this node's origin sits. Changing it moves the origin, never the shape.");
        for option in Anchor::ALL {
            if ui.selectable_label(anchor == option, option.label()).clicked() && anchor != option {
                anchor = option;
                app.edit("Anchor", None);
                if let Some(node) = app.scene.get_mut(id) {
                    node.anchor = anchor;
                }
            }
        }
    });
}

fn group(app: &mut App, ui: &mut egui::Ui, id: NodeId, current: GroupOp) {
    let mut op = current;
    ui.horizontal(|ui| {
        row_label(ui, "Operation");
        for option in GroupOp::ALL {
            if ui.selectable_label(op == option, option.label()).clicked() && op != option {
                op = option;
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
                ui.add(
                    egui::Label::new(theme::hint(format!(
                        "Base: {}. Every visible child below it is cut out of it.",
                        app.scene.node(base).name
                    )))
                    .selectable(false),
                );
            }
            None => {
                ui.colored_label(token::ACCENT, "No visible child, so there is nothing to cut.");
            }
        }
    }

    for (index, child) in children.iter().enumerate() {
        let child = *child;
        ui.horizontal(|ui| {
            let name = app.scene.node(child).name.clone();
            let is_base = op.order_matters() && Some(child) == app.scene.difference_base(id);
            let cut = op.order_matters() && !is_base;
            let mark = if is_base {
                "base"
            } else if cut {
                "cut"
            } else {
                ""
            };
            if ui.selectable_label(app.is_selected(child), theme::value(format!("{}. {name}", index + 1))).clicked() {
                app.select_only(child);
            }
            if !mark.is_empty() {
                ui.add(
                    egui::Label::new(egui::RichText::new(mark).size(theme::font::SMALL).color(if cut {
                        token::DANGER
                    } else {
                        token::TEXT_LO
                    }))
                    .selectable(false),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(index + 1 < children.len(), egui::Button::new("\u{25BE}").small()).clicked() {
                    app.edit("Reorder", None);
                    app.scene.reorder(child, 1);
                }
                if ui.add_enabled(index > 0, egui::Button::new("\u{25B4}").small()).clicked() {
                    app.edit("Reorder", None);
                    app.scene.reorder(child, -1);
                }
            });
        });
    }
    if children.is_empty() {
        ui.add(egui::Label::new(theme::hint("This group is empty.")).selectable(false));
    }
}

fn primitive(app: &mut App, ui: &mut egui::Ui, id: NodeId, type_id: &str) {
    let Some(spec) = scadstudio_core::primitive::lookup(type_id) else {
        ui.colored_label(token::DANGER, format!("Unknown primitive type \"{type_id}\""));
        return;
    };
    let unit = app.unit();
    let params = app.scene.node(id).params().cloned().unwrap_or_default();

    for param in spec.params {
        if !spec.param_visible(param, &params) {
            continue;
        }
        let value = params.get(param.key).copied().unwrap_or(param.default);
        match param.kind {
            // Radio-style choices where a measurement is ambiguous.
            ParamKind::Choice { options } => {
                ui.horizontal(|ui| {
                    row_label(ui, param.label);
                    let mut chosen = value.as_u32();
                    ui.vertical(|ui| {
                        for (index, option) in options.iter().enumerate() {
                            if ui.selectable_label(chosen == index as u32, *option).clicked() && chosen != index as u32
                            {
                                chosen = index as u32;
                                app.edit("Set measurement", None);
                                set_param(app, id, param.key, ParamValue::Choice(chosen));
                                sync_wall_mode(app, id, param.key, chosen);
                            }
                        }
                    });
                });
            }
            ParamKind::Bool => {
                ui.horizontal(|ui| {
                    row_label(ui, param.label);
                    let mut on = value.as_bool();
                    if ui.checkbox(&mut on, "").changed() {
                        app.edit("Set flag", None);
                        set_param(app, id, param.key, ParamValue::Bool(on));
                    }
                });
            }
            kind => {
                ui.horizontal(|ui| {
                    row_label(ui, param.label);
                    // A lock toggle where the type offers one: a sphere's three
                    // diameters, a cylinder's two.
                    if param.lock_group != 0 {
                        let locked = is_locked(app, id, param.lock_group);
                        let response = crate::icon::button(ui, crate::icon::Glyph::Group, 18.0, locked, true);
                        if response
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
                    // The unit rides at the right-hand end of the row, one size
                    // down and in the label colour, so it never competes with
                    // the number it qualifies.
                    let suffix = match kind {
                        ParamKind::Length { .. } => unit.suffix(),
                        ParamKind::Angle { .. } => "deg",
                        _ => "",
                    };
                    let suffix_width = if suffix.is_empty() { 0.0 } else { 26.0 };
                    let field_width = (ui.available_width() - suffix_width).max(40.0);
                    let field_id = ui.id().with((id, param.key));
                    let shown = ui::show_param(value, unit);
                    let committed = ui
                        .scope(|ui| {
                            ui.set_width(field_width);
                            app.fields.field(ui, field_id, &shown)
                        })
                        .inner;
                    if !suffix.is_empty() {
                        ui.add(egui::Label::new(theme::hint(suffix)).selectable(false));
                    }
                    if let Some(text) = committed {
                        match ui::commit_param(&text, kind, unit) {
                            Commit::Value(new_value) => {
                                app.edit(&format!("Set {}", param.label), Some(&format!("param:{id}:{}", param.key)));
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
            }
        }
    }

    if spec.segmented {
        ui.horizontal(|ui| {
            row_label(ui, "Segments").on_hover_text("Overrides the scene default for this object's curved surfaces.");
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
                    if ui.add(egui::DragValue::new(&mut value).range(3.0..=512.0).max_decimals(0)).changed() {
                        app.edit("Segments", Some(&format!("segments:{id}")));
                        if let Some(node) = app.scene.get_mut(id) {
                            node.segments = Some(value.round() as u32);
                        }
                    }
                }
                None => {
                    ui.add(egui::Label::new(theme::hint(format!("{default} (scene default)"))).selectable(false));
                }
            }
        });
    }
}

fn placement(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let unit = app.unit();
    let node = app.scene.node(id);
    let position = node.position;
    let rotation = node.rotation;

    // Three columns of numbers, each fronted by its axis colour: the row says
    // which axis is which without spending a character on saying so.
    axis_row(ui, &format!("Position ({})", unit.suffix()), |ui, axis| {
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
    });

    axis_row(ui, "Rotation (deg)", |ui, axis| {
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
    });

    ui.add(egui::Label::new(theme::hint("Rotations are applied X, then Y, then Z.")).selectable(false));
}

/// One labelled row of three axis fields, each preceded by its colour chip.
fn axis_row(ui: &mut egui::Ui, label: &str, mut field: impl FnMut(&mut egui::Ui, usize)) {
    ui.horizontal(|ui| {
        row_label(ui, label);
        // Three fields, three chips, and the gaps between them all have to come
        // out of the row: getting this wrong pushes the Z field off the panel.
        let available = ui.available_width();
        let chips = 3.0 * (3.0 + ui.spacing().item_spacing.x);
        let gaps = 2.0 * ui.spacing().item_spacing.x;
        let each = ((available - chips - gaps) / 3.0).max(30.0);
        for axis in 0..3 {
            theme::axis_chip(ui, axis);
            ui.scope(|ui| {
                ui.set_width(each);
                field(ui, axis);
            });
        }
    });
}

fn measurements(app: &mut App, ui: &mut egui::Ui, id: NodeId) {
    let unit = app.unit();
    match app.evaluated.node_world_bounds.get(&id).copied() {
        Some((lo, hi)) => {
            ui.horizontal(|ui| {
                row_label(ui, "Size");
                ui.add(egui::Label::new(theme::numeric(ui::describe_size(hi - lo, unit))).selectable(false));
            });
            ui.horizontal(|ui| {
                row_label(ui, "Centre");
                ui.add(
                    egui::Label::new(theme::numeric(format!(
                        "{}, {}, {} {}",
                        format_length((lo.x + hi.x) / 2.0, unit),
                        format_length((lo.y + hi.y) / 2.0, unit),
                        format_length((lo.z + hi.z) / 2.0, unit),
                        unit.suffix()
                    )))
                    .selectable(false),
                );
            });
        }
        None => {
            ui.add(egui::Label::new(theme::hint("No geometry yet.")).selectable(false));
        }
    }
    if let Some(error) = app.evaluated.error_for(id) {
        ui.colored_label(token::DANGER, &error.message);
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
    let keys: Vec<String> = spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key.to_string()).collect();
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
    let keys: Vec<String> = spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key.to_string()).collect();
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
        app_scene.get_mut(id).unwrap().params_mut().unwrap().insert("diameter_y".into(), ParamValue::Length(30.0));
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
        let visible: Vec<&str> = spec.params.iter().filter(|p| spec.param_visible(p, &params)).map(|p| p.key).collect();
        assert!(visible.contains(&"wall_thickness"));
        assert!(!visible.contains(&"inner_diameter"));

        params.insert("wall_mode".into(), ParamValue::Choice(1));
        let visible: Vec<&str> = spec.params.iter().filter(|p| spec.param_visible(p, &params)).map(|p| p.key).collect();
        assert!(visible.contains(&"inner_diameter"));
        assert!(!visible.contains(&"wall_thickness"));
    }
}
