//! One parameter's field, whichever kind of parameter it is.

use super::*;
use crate::app::App;
use crate::theme::{self};
use crate::ui::{self};
use simple3d_core::primitive::{ParamKind, ParamValue};
use simple3d_core::scene::NodeId;
use simple3d_core::unit::Unit;

/// Render one parameter's row -- a choice, a checkbox or a number field --
/// writing edits to every selected node. Shared by the primitive editor and the
/// pattern editor (issue 67), which drive it from different parameter lists.
pub(crate) fn param_field(
    app: &mut App,
    ui: &mut egui::Ui,
    targets: &[NodeId],
    id: NodeId,
    param: &simple3d_core::primitive::ParamSpec,
    unit: Unit,
    style: RowStyle,
) {
    param_field_as(app, ui, targets, id, param, param.label, unit, style);
}

/// The same row, drawn under a name of the caller's choosing.
///
/// A parameter's label is two things at once: what the row reads as, and what
/// its value field answers to across a relayout (see [`grip_id`]). Those pull
/// apart in exactly one place -- the pattern tool's stages, where four stages
/// each have a "Copies" and the labels must therefore carry a stage number that
/// the stage's own heading has already said (issue 79). The grip goes on using
/// `param.label`; only what is drawn changes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn param_field_as(
    app: &mut App,
    ui: &mut egui::Ui,
    targets: &[NodeId],
    id: NodeId,
    param: &simple3d_core::primitive::ParamSpec,
    shown_as: &str,
    unit: Unit,
    style: RowStyle,
) {
    let value = param_value(app, id, param.key, param.default);
    match param.kind {
        // Radio-style choices where a measurement is ambiguous.
        // The options flow across the row and wrap when they run out of it,
        // rather than each taking a line of its own. A pattern's kind is six
        // choices and its axis is three, and stacked they pushed everything
        // below them -- the numbers those choices govern -- off the bottom of
        // the panel. Wrapped, a narrow panel still ends up with one per line,
        // which is the layout this replaces, so nothing is lost at any width.
        ParamKind::Choice { options } => {
            field_row(ui, shown_as, "", |ui| {
                let mut chosen = value.as_u32();
                for (index, option) in options.iter().enumerate() {
                    if theme::choice(ui, chosen == index as u32, option).clicked() && chosen != index as u32 {
                        chosen = index as u32;
                        app.edit(style.edit_label, None);
                        for target in targets {
                            set_param(app, *target, param.key, ParamValue::Choice(chosen));
                            sync_wall_mode(app, *target, param.key, chosen);
                        }
                    }
                }
            });
        }
        ParamKind::Bool => {
            field_row(ui, shown_as, "", |ui| {
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
            // Worked out before the row is laid out, because the unit is part
            // of the row's name.
            let name = named(
                shown_as,
                match kind {
                    ParamKind::Length { .. } => unit.suffix(),
                    ParamKind::Angle { .. } => "deg",
                    _ => "",
                },
            );
            field_row(ui, &name, "", |ui| {
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
                // The unit is in the name, so there is nothing to leave room for
                // and the field takes the whole column.
                let field_width = room_left(ui).max(40.0);
                let field_id = ui.id().with((id, param.key));
                ui.scope(|ui| {
                    ui.set_width(field_width);
                    number_field(app, ui, targets, param, unit, style, field_id);
                });
            });
        }
    }
}

/// Just the value field of a number parameter, as wide as the ui it is given,
/// with no row around it.
///
/// For rows that put several numbers side by side under one name -- a pattern
/// stage's step, a shift, the scatter's nudge (issue 79) -- as well as for the
/// ordinary row above, which is this field after a name and a lock.
#[allow(clippy::too_many_arguments)]
pub(crate) fn number_field(
    app: &mut App,
    ui: &mut egui::Ui,
    targets: &[NodeId],
    param: &simple3d_core::primitive::ParamSpec,
    unit: Unit,
    style: RowStyle,
    field_id: egui::Id,
) {
    let kind = param.kind;
    let step = ui::scrub_increment(kind, unit);
    // With several nodes selected, a field shows the value they agree on and
    // an em dash when they do not.
    let shown =
        ui::shared_text(targets.iter().map(|t| ui::show_param(param_value(app, *t, param.key, param.default), unit)));
    // The field is the grip: dragging it changes the value without going near
    // the keyboard, and clicking it opens it for typing.
    let grip_name = if style.grip_scope.is_empty() {
        param.label.to_string()
    } else {
        format!("{}:{}", style.grip_scope, param.label)
    };
    let outcome = value_field(app, ui, &grip_name, field_id, &shown, step);
    if let Some(scrubbed) = outcome.scrubbed {
        scrub_param(app, targets, param, kind, unit, scrubbed.delta, scrubbed.started);
    }
    if let Some(text) = outcome.committed {
        set_shared_param(app, targets, param, kind, unit, field_id, text);
    }
}

/// Three of a pattern's numbers on one row under one name -- a stage's step,
/// a shift, the scatter's nudge -- the way a position's three are, and one to a
/// line where the row is too narrow for three (issue 79).
///
/// They were three rows each, "Step X", "Step Y", "Step Z", which is three
/// names for one thing and a column three times as tall as what it says.
#[allow(clippy::too_many_arguments)]
pub(crate) fn vector_row(
    app: &mut App,
    ui: &mut egui::Ui,
    id: NodeId,
    keys: [&'static str; 3],
    name: &str,
    hover: &str,
    style: RowStyle,
) {
    let unit = app.unit();
    let specs: Vec<&'static simple3d_core::primitive::ParamSpec> =
        keys.iter().filter_map(|key| simple3d_core::pattern::param_spec(key)).collect();
    if specs.len() != 3 {
        return;
    }
    // The axis chips are named after the first value rather than the row's
    // name: every stage has a "Step", and chips named after that clashed.
    let chips = format!("{}:{}", style.grip_scope, keys[0]);
    field_row(ui, &named(name, unit.suffix()), hover, |ui| {
        point_fields(ui, &chips, |ui, axis| {
            let spec = specs[axis];
            // Named after the value rather than where it sits, and told apart
            // by the scope, so the tool and the noise window can show the same
            // number in one frame.
            let field_id = egui::Id::new(("pattern-vector", id, spec.key, style.grip_scope));
            number_field(app, ui, &[id], spec, unit, style, field_id);
        });
    });
}
