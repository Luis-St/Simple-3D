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

/// What a scrub gesture did this frame.
struct Scrubbed {
    /// True on the frame the drag began: the one frame that takes an undo
    /// snapshot, so the whole drag collapses into a single step.
    started: bool,
    /// Change to apply, in the unit the field displays.
    delta: f64,
}

/// A label the value can be dragged from. Returns `Some` on every frame of a
/// drag, including the first.
///
/// The label is the grip rather than the field itself because the field has to
/// stay a text field: a click in it must put a caret where it was clicked, and a
/// drag in it must select text. The label next to it has no such job, which is
/// what makes it free to carry the gesture.
fn scrub_grip(app: &mut App, ui: &mut egui::Ui, response: &egui::Response, step: f64) -> Option<Scrubbed> {
    let id = response.id;
    if response.hovered() || app.scrub.id == Some(id) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if response.drag_started() {
        app.scrub.id = Some(id);
    }
    if app.scrub.id != Some(id) {
        return None;
    }
    if !response.dragged() {
        app.scrub.id = None;
        return None;
    }
    let (fine, coarse) = ui.input(|i| (i.modifiers.shift, i.modifiers.command));
    Some(Scrubbed {
        started: response.drag_started(),
        delta: ui::scrub_delta(response.drag_delta().x, step, fine, coarse),
    })
}

/// A label that scrubs: the same row label, but sensing a drag.
fn scrub_label(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(LABEL_WIDTH, theme::metric::INPUT_ROW), egui::Sense::drag());
    let colour = if response.hovered() || response.dragged() { token::TEXT_HI } else { token::TEXT_LO };
    ui.painter().text(
        egui::pos2(rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(theme::font::LABEL),
        colour,
    );
    response
}

/// The panel's contents, without the dock around them, so the same panel can be
/// drawn in either dock.
pub fn show_inside(app: &mut App, ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(theme::metric::GAP, 2.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
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
                let label = scadstudio_core::primitive::lookup(&type_id).map(|s| s.label).unwrap_or("");
                let note =
                    if targets.len() > 1 { format!("{} \u{00D7} {label}", targets.len()) } else { label.to_string() };
                section_titled(ui, "Dimensions", &note, |ui| primitive(app, ui, &targets, &type_id));
            }
            None => match app.scene.node(primary).body.clone() {
                Body::Group { op } if targets.len() == 1 => section(ui, "Boolean", |ui| group(app, ui, primary, op)),
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
}

/// The primitive type every selected node has, or `None` when they are not all
/// the same kind of thing. This is what decides whether a Dimensions panel can
/// speak for the whole selection.
fn shared_type(app: &App, targets: &[NodeId]) -> Option<String> {
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

fn common(app: &mut App, ui: &mut egui::Ui, targets: &[NodeId]) {
    let Some(&id) = targets.last() else { return };
    let node = app.scene.node(id);
    let is_root = id == app.scene.root();
    let mut name = node.name.clone();
    let mut visible = targets.iter().all(|t| app.scene.node(*t).visible);
    let mut anchor = node.anchor;
    let many = targets.len() > 1;

    ui.horizontal(|ui| {
        row_label(ui, "Name");
        if many {
            // Renaming several nodes to one name would make the outliner
            // unreadable, so the field says what is selected instead.
            ui.add(egui::Label::new(theme::value(format!("{} objects selected", targets.len()))).selectable(false));
        } else if ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY)).changed() {
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
            for target in targets {
                if let Some(node) = app.scene.get_mut(*target) {
                    node.visible = visible;
                }
            }
        }
    });

    ui.horizontal(|ui| {
        row_label(ui, "Anchor")
            .on_hover_text("Where this node's origin sits. Changing it moves the origin, never the shape.");
        let mixed = targets.iter().any(|t| app.scene.node(*t).anchor != anchor);
        for option in Anchor::ALL {
            let showing = !mixed && anchor == option;
            if ui.selectable_label(showing, option.label()).clicked() && (mixed || anchor != option) {
                anchor = option;
                app.edit("Anchor", None);
                for target in targets {
                    if let Some(node) = app.scene.get_mut(*target) {
                        node.anchor = anchor;
                    }
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

/// What one node currently holds for a parameter.
fn param_value(app: &App, id: NodeId, key: &str, default: ParamValue) -> ParamValue {
    app.scene.node(id).params().and_then(|p| p.get(key).copied()).unwrap_or(default)
}

fn primitive(app: &mut App, ui: &mut egui::Ui, targets: &[NodeId], type_id: &str) {
    let Some(spec) = scadstudio_core::primitive::lookup(type_id) else {
        ui.colored_label(token::DANGER, format!("Unknown primitive type \"{type_id}\""));
        return;
    };
    let Some(&id) = targets.last() else { return };
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
                                for target in targets {
                                    set_param(app, *target, param.key, ParamValue::Choice(chosen));
                                    sync_wall_mode(app, *target, param.key, chosen);
                                }
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
                        for target in targets {
                            set_param(app, *target, param.key, ParamValue::Bool(on));
                        }
                    }
                });
            }
            kind => {
                ui.horizontal(|ui| {
                    // The label is the scrub grip: dragging it changes the value
                    // without going near the keyboard.
                    let grip = scrub_label(ui, param.label);
                    let step = ui::scrub_increment(kind, unit);
                    if let Some(scrubbed) = scrub_grip(app, ui, &grip, step) {
                        scrub_param(app, targets, param, kind, unit, scrubbed.delta, scrubbed.started);
                    }
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
                    // With several nodes selected, a field shows the value they
                    // agree on and an em dash when they do not.
                    let shown = ui::shared_text(
                        targets.iter().map(|t| ui::show_param(param_value(app, *t, param.key, param.default), unit)),
                    );
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
                        set_shared_param(app, targets, param, kind, unit, field_id, text);
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
                for target in targets {
                    if let Some(node) = app.scene.get_mut(*target) {
                        node.segments = if overridden { Some(default) } else { None };
                    }
                }
            }
            let default = app.scene.settings.default_segments;
            match app.scene.node(id).segments {
                Some(current) => {
                    let mut value = current as f64;
                    if ui.add(egui::DragValue::new(&mut value).range(3.0..=512.0).max_decimals(0)).changed() {
                        app.edit("Segments", Some(&format!("segments:{id}")));
                        for target in targets {
                            if let Some(node) = app.scene.get_mut(*target) {
                                node.segments = Some(value.round() as u32);
                            }
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

/// Apply one typed value to every selected node.
///
/// An absolute entry gives them all the same number; a delta (`+2`) is resolved
/// against each node's own value, which is the whole point of having one --
/// "two millimetres wider" means something different for every shape it is
/// applied to.
///
/// A value none of them can read leaves every one of them alone and marks the
/// field. Nothing partial: the selection does not end up half-edited.
fn set_shared_param(
    app: &mut App,
    targets: &[NodeId],
    param: &scadstudio_core::primitive::ParamSpec,
    kind: ParamKind,
    unit: Unit,
    field_id: egui::Id,
    text: String,
) {
    // The em dash is what the field shows for a disagreement; leaving it there
    // and tabbing away must not write it to anything.
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

fn placement(app: &mut App, ui: &mut egui::Ui, targets: &[NodeId]) {
    let unit = app.unit();
    let Some(&primary) = targets.last() else { return };

    // Three columns of numbers, each fronted by its axis colour: the row says
    // which axis is which without spending a character on saying so. The chip
    // is also that field's scrub grip -- it is the only label the field has.
    axis_row(app, ui, &format!("Position ({})", unit.suffix()), |app, ui, axis, grip| {
        if let Some(scrubbed) = scrub_grip(app, ui, grip, unit.from_mm(app.move_snap())) {
            scrub_transform(app, targets, axis, unit.to_mm(scrubbed.delta), false, scrubbed.started);
        }
        let field_id = ui.id().with((primary, "pos", axis));
        let shown =
            ui::shared_text(targets.iter().map(|t| format_length(component(app.scene.node(*t).position, axis), unit)));
        if let Some(text) = app.fields.field(ui, field_id, &shown) {
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

    axis_row(app, ui, "Rotation (deg)", |app, ui, axis, grip| {
        if let Some(scrubbed) = scrub_grip(app, ui, grip, app.settings.rotate_snap_deg.max(1.0)) {
            scrub_transform(app, targets, axis, scrubbed.delta, true, scrubbed.started);
        }
        let field_id = ui.id().with((primary, "rot", axis));
        let shown = ui::shared_text(targets.iter().map(|t| format_angle(component(app.scene.node(*t).rotation, axis))));
        if let Some(text) = app.fields.field(ui, field_id, &shown) {
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

/// One labelled row of three axis fields, each preceded by its colour chip. The
/// chip is handed to the caller as that field's scrub grip.
fn axis_row(
    app: &mut App,
    ui: &mut egui::Ui,
    label: &str,
    mut field: impl FnMut(&mut App, &mut egui::Ui, usize, &egui::Response),
) {
    ui.horizontal(|ui| {
        row_label(ui, label);
        // Three fields, three chips, and the gaps between them all have to come
        // out of the row: getting this wrong pushes the Z field off the panel.
        let available = ui.available_width();
        let chips = 3.0 * (theme::AXIS_CHIP_WIDTH + ui.spacing().item_spacing.x);
        let gaps = 2.0 * ui.spacing().item_spacing.x;
        let each = ((available - chips - gaps) / 3.0).max(30.0);
        for axis in 0..3 {
            let grip = theme::axis_chip(ui, axis);
            ui.scope(|ui| {
                ui.set_width(each);
                field(app, ui, axis, &grip);
            });
        }
    });
}

/// One frame of a scrub on a dimension.
///
/// `started` is the only frame that takes an undo snapshot. Every frame after
/// it goes through `touch`, which re-evaluates without recording -- so a drag
/// across forty pixels is one step to undo, not forty.
fn scrub_param(
    app: &mut App,
    targets: &[NodeId],
    param: &scadstudio_core::primitive::ParamSpec,
    kind: ParamKind,
    unit: Unit,
    delta: f64,
    started: bool,
) {
    if started {
        app.edit(&format!("Scrub {}", param.label), None);
    }
    for target in targets {
        let current = ui::param_number(param_value(app, *target, param.key, param.default));
        let shown = match kind {
            ParamKind::Length { .. } => unit.from_mm(current),
            _ => current,
        };
        let next = ui::value_from_display(kind, unit, shown + delta);
        set_param(app, *target, param.key, next);
        apply_lock(app, *target, param.lock_group, param.key, next);
    }
    app.touch();
    app.fields.clear();
}

/// One frame of a scrub on a position (millimetres) or a rotation (degrees).
fn scrub_transform(app: &mut App, targets: &[NodeId], axis: usize, delta: f64, rotation: bool, started: bool) {
    if started {
        app.edit(if rotation { "Scrub rotation" } else { "Scrub position" }, None);
    }
    for target in targets {
        let Some(node) = app.scene.get_mut(*target) else { continue };
        let mut v = if rotation { node.rotation } else { node.position };
        let next = crate::gizmo::get_axis(v, axis) + delta;
        set_component(&mut v, axis, next);
        if rotation {
            node.rotation = v;
        } else {
            node.position = v;
        }
    }
    app.touch();
    app.fields.clear();
}

fn measurements(app: &mut App, ui: &mut egui::Ui, id: NodeId, selected: usize) {
    if selected > 1 {
        ui.add(
            egui::Label::new(theme::hint(format!("Measured from {}, the last one selected.", app.scene.node(id).name)))
                .selectable(false),
        );
    }
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

    /// An `App` on a headless context, pointed at a throwaway config directory
    /// so a test cannot read or write the developer's own.
    fn headless_app() -> App {
        let dir = std::env::temp_dir().join(format!(
            "scadstudio-props-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        App::with_config_dir(&egui::Context::default(), None, dir)
    }

    /// Two plates of different widths, selected together.
    fn two_plates(app: &mut App) -> (NodeId, NodeId) {
        let root = app.scene.root();
        let a = app.scene.add_primitive("plate", root, 0).unwrap();
        let b = app.scene.add_primitive("plate", root, 1).unwrap();
        set_param(app, a, "width", ParamValue::Length(40.0));
        set_param(app, b, "width", ParamValue::Length(60.0));
        app.selection = vec![a, b];
        (a, b)
    }

    fn width_of(app: &App, id: NodeId) -> f64 {
        app.scene.node(id).params().unwrap().num("width")
    }

    fn width_spec() -> &'static primitive::ParamSpec {
        primitive::lookup("plate").unwrap().params.iter().find(|p| p.key == "width").unwrap()
    }

    #[test]
    fn a_field_over_a_multi_selection_shows_the_shared_value_or_an_em_dash() {
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        let unit = app.unit();
        let shown = |app: &App, ids: &[NodeId]| {
            ui::shared_text(
                ids.iter().map(|t| ui::show_param(param_value(app, *t, "width", ParamValue::Length(0.0)), unit)),
            )
        };
        assert_eq!(shown(&app, &[a, b]), ui::MIXED, "two different widths must not claim to be one");
        assert_eq!(shown(&app, &[a]), "40");
        set_param(&mut app, b, "width", ParamValue::Length(40.0));
        assert_eq!(shown(&app, &[a, b]), "40", "two equal widths are one value, not a dash");
    }

    #[test]
    fn an_absolute_value_applies_to_the_whole_selection_and_a_delta_applies_per_node() {
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        let unit = app.unit();
        let param = width_spec();
        let field = egui::Id::new("width-field");

        set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "25".into());
        assert_eq!((width_of(&app, a), width_of(&app, b)), (25.0, 25.0), "an absolute value is one value for all");

        set_param(&mut app, a, "width", ParamValue::Length(40.0));
        set_param(&mut app, b, "width", ParamValue::Length(60.0));
        set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "+2".into());
        assert_eq!(
            (width_of(&app, a), width_of(&app, b)),
            (42.0, 62.0),
            "a delta is relative to each node's own value"
        );

        // And the field's other tricks reach the model the same way.
        set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "4cm".into());
        assert_eq!((width_of(&app, a), width_of(&app, b)), (40.0, 40.0), "a value in another unit did not convert");
        set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "12+8".into());
        assert_eq!((width_of(&app, a), width_of(&app, b)), (20.0, 20.0), "an expression was not evaluated");
    }

    #[test]
    fn an_em_dash_left_alone_edits_nothing() {
        // Tabbing through a panel of mixed values must not flatten them.
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        let unit = app.unit();
        let param = width_spec();
        set_shared_param(&mut app, &[a, b], param, param.kind, unit, egui::Id::new("f"), ui::MIXED.into());
        assert_eq!((width_of(&app, a), width_of(&app, b)), (40.0, 60.0));
    }

    #[test]
    fn a_value_that_cannot_be_read_leaves_every_node_alone_and_marks_the_field() {
        // Acceptance criterion 14, and the design's rule that the typed text
        // stays put: it is the thing the user has to correct.
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        let unit = app.unit();
        let param = width_spec();
        let field = egui::Id::new("width-field");
        let before = app.history.revision();

        set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "wide-ish".into());
        assert_eq!((width_of(&app, a), width_of(&app, b)), (40.0, 60.0), "a rejected entry changed the model");
        assert_eq!(app.history.revision(), before, "a rejected entry took an undo step");
        assert!(app.fields.is_rejected(field), "the field was not marked");

        // Correcting it clears the mark.
        set_shared_param(&mut app, &[a, b], param, param.kind, unit, field, "30".into());
        assert!(!app.fields.is_rejected(field));
        assert_eq!((width_of(&app, a), width_of(&app, b)), (30.0, 30.0));
    }

    #[test]
    fn a_whole_scrub_is_one_undo_step() {
        // Forty snapshots for one drag would make undo useless exactly where it
        // is needed most.
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        let unit = app.unit();
        let param = width_spec();
        let steps = app.history.undo_len();

        for frame in 0..40 {
            scrub_param(&mut app, &[a, b], param, param.kind, unit, 0.5, frame == 0);
        }
        assert_eq!(width_of(&app, a), 60.0, "the scrub did not accumulate");
        assert_eq!(width_of(&app, b), 80.0, "each node scrubs from its own value");
        assert_eq!(app.history.undo_len(), steps + 1, "the drag left more than one step to undo");

        app.history.undo(&mut app.scene);
        assert_eq!((width_of(&app, a), width_of(&app, b)), (40.0, 60.0), "one undo did not put the drag back");
    }

    #[test]
    fn a_position_scrub_moves_every_selected_node_by_the_same_amount() {
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        app.scene.get_mut(b).unwrap().position = Vec3::new(10.0, 0.0, 0.0);
        for frame in 0..10 {
            scrub_transform(&mut app, &[a, b], 0, 1.0, false, frame == 0);
        }
        assert_eq!(app.scene.node(a).position.x, 10.0);
        assert_eq!(app.scene.node(b).position.x, 20.0);
        app.history.undo(&mut app.scene);
        assert_eq!(app.scene.node(a).position.x, 0.0);
        assert_eq!(app.scene.node(b).position.x, 10.0);
    }

    #[test]
    fn a_selection_of_one_kind_of_shape_gets_a_dimensions_panel_and_a_mixed_one_does_not() {
        let mut app = headless_app();
        let (a, b) = two_plates(&mut app);
        assert_eq!(shared_type(&app, &[a, b]).as_deref(), Some("plate"));
        let root = app.scene.root();
        let sphere = app.scene.add_primitive("sphere", root, 2).unwrap();
        assert_eq!(shared_type(&app, &[a, sphere]), None, "a plate and a sphere share no dimension");
        let group = app.scene.add_group(GroupOp::Union, root, 3);
        assert_eq!(shared_type(&app, &[group]), None, "a group has no dimensions of its own");
    }

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
