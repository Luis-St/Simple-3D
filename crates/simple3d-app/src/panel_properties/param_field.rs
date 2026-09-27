//! One parameter's field, whatever its kind.

use super::*;
use crate::app::App;
use crate::theme::{self};
use crate::ui::{self};
use simple3d_core::primitive::{ParamKind, ParamValue};
use simple3d_core::scene::NodeId;
use simple3d_core::unit::Unit;

/// One parameter row (choice, checkbox or number), writing edits to every selected node. Shared by
/// the primitive and pattern editors (issue 67).
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

/// The same row under a caller-chosen name. The grip still uses `param.label`, so the pattern
/// tool can draw "Copies" while the grip keeps the stage number (issue 79).
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
    param_field_on(app, ui, targets, id, param, shown_as, unit, style, None);
}

/// The same row with the axis chip before a number field (issue 110): `None` for no chip column,
/// an empty list for a dimension along no axis (keeping alignment).
#[allow(clippy::too_many_arguments)]
pub(crate) fn param_field_on(
    app: &mut App,
    ui: &mut egui::Ui,
    targets: &[NodeId],
    id: NodeId,
    param: &simple3d_core::primitive::ParamSpec,
    shown_as: &str,
    unit: Unit,
    style: RowStyle,
    axes: Option<&[usize]>,
) {
    let value = param_value(app, id, param.key, param.default);
    match param.kind {
        // Choices flow across the row and wrap, rather than one per line pushing the numbers off the panel.
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
            // Computed before layout, since the unit is part of the row's name.
            let name = named(
                shown_as,
                match kind {
                    ParamKind::Length { .. } => unit.suffix(),
                    ParamKind::Angle { .. } => "deg",
                    _ => "",
                },
            );
            field_row(ui, &name, "", |ui| {
                if let Some(axes) = axes {
                    theme::axes_chip(ui, axes);
                }
                // A lock toggle where the type offers one: a sphere's three diameters, a cylinder's two.
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
                // The unit is in the name, so the field takes the whole column.
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

/// Just a number parameter's value field, as wide as the given ui, for rows with several numbers
/// under one name (issue 79) and for the ordinary row.
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
    // With several nodes selected, show the agreed value or an em dash.
    let shown =
        ui::shared_text(targets.iter().map(|t| ui::show_param(param_value(app, *t, param.key, param.default), unit)));
    // The field is the grip: drag to change, click to type.
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

/// Three pattern numbers on one row under one name, one per line when narrow (issue 79).
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
    // Chips are named after the first value, since every stage has a "Step" and names clashed.
    let chips = format!("{}:{}", style.grip_scope, keys[0]);
    field_row(ui, &named(name, unit.suffix()), hover, |ui| {
        point_fields(ui, &chips, |ui, axis| {
            let spec = specs[axis];
            // Named by value and scope, so the tool and the noise window can show the same number in one frame.
            let field_id = egui::Id::new(("pattern-vector", id, spec.key, style.grip_scope));
            number_field(app, ui, &[id], spec, unit, style, field_id);
        });
    });
}
