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
            field_row(ui, param.label, "", |ui| {
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
            field_row(ui, param.label, "", |ui| {
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
                param.label,
                match kind {
                    ParamKind::Length { .. } => unit.suffix(),
                    ParamKind::Angle { .. } => "deg",
                    _ => "",
                },
            );
            field_row(ui, &name, "", |ui| {
                let step = ui::scrub_increment(kind, unit);
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
                // With several nodes selected, a field shows the value they
                // agree on and an em dash when they do not.
                let shown = ui::shared_text(
                    targets.iter().map(|t| ui::show_param(param_value(app, *t, param.key, param.default), unit)),
                );
                // The field is the grip: dragging it changes the value
                // without going near the keyboard, and clicking it opens it
                // for typing.
                let grip_name = if style.grip_scope.is_empty() {
                    param.label.to_string()
                } else {
                    format!("{}:{}", style.grip_scope, param.label)
                };
                let outcome = ui
                    .scope(|ui| {
                        ui.set_width(field_width);
                        value_field(app, ui, &grip_name, field_id, &shown, step)
                    })
                    .inner;
                if let Some(scrubbed) = outcome.scrubbed {
                    scrub_param(app, targets, param, kind, unit, scrubbed.delta, scrubbed.started);
                }
                if let Some(text) = outcome.committed {
                    set_shared_param(app, targets, param, kind, unit, field_id, text);
                }
            });
        }
    }
}
