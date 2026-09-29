//! A popup's setting rows: a labelled name and a drag-or-type number, shared by every tool so they
//! behave like the properties panel.

use crate::app::{App, Status};
use crate::ui;
use simple3d_core::primitive::ParamKind;

/// A row's name; its hover sits on the label, since a tooltip on the field would get in a drag's way.
pub(crate) fn label(ui: &mut egui::Ui, name: &str, hover: &str) {
    ui.label(name).on_hover_text(hover);
}

/// A drag-or-type number, through the properties panel's buffers. Not a document parameter, so no
/// undo or re-evaluation: the value goes into the tool's plan. `scope` and `name` separate fields.
pub(crate) fn number(
    app: &mut App,
    ui: &mut egui::Ui,
    (scope, name): (&str, &str),
    width: f32,
    kind: ParamKind,
    value: &mut f64,
) {
    let unit = app.unit();
    let shown = match kind {
        ParamKind::Length { .. } => simple3d_core::unit::format_length(*value, unit),
        ParamKind::Angle { .. } => simple3d_core::unit::format_angle(*value),
        // Percentages and counts are whole numbers.
        _ => format!("{}", value.round() as i64),
    };
    let id = egui::Id::new((scope, name));
    let step = ui::scrub_increment(kind, unit);
    // The scrub state is lifted out so the field can borrow the buffers without borrowing the app twice.
    let mut scrub = app.scrub;
    let outcome = ui
        .scope(|ui| {
            ui.set_max_width(width);
            app.fields.scrub_field(ui, id, crate::panel_properties::grip_id(name), &shown, step, &mut scrub)
        })
        .inner;
    app.scrub = scrub;
    if let Some(scrubbed) = outcome.scrubbed {
        // Whole numbers carry the fraction between frames, as the properties panel does; rounding each
        // frame's small movement away made a count need a long drag per step.
        let whole = matches!(kind, ParamKind::Count { .. });
        let carried = if whole { app.scrub.carry } else { 0.0 };
        let displayed = match kind {
            ParamKind::Length { .. } => unit.from_mm(*value),
            _ => *value,
        };
        let wanted = displayed + scrubbed.delta + carried;
        *value = ui::param_number(ui::value_from_display(kind, unit, wanted));
        if whole {
            app.scrub.carry = (wanted - *value).clamp(-1.0, 1.0);
        }
    }
    if let Some(text) = outcome.committed {
        match ui::commit_param(&text, kind, unit, *value) {
            ui::Commit::Value(committed) => {
                app.fields.accept(id);
                *value = ui::param_number(committed);
            }
            ui::Commit::Revert => {
                app.fields.reject(id, text.clone());
                app.status = Status::Info(format!("\"{text}\" is not a number this field can take"));
            }
        }
    }
}

/// The painted cross that drops one entry of a list, as the UI font may lack the glyph; `label` names
/// it for the accessibility tree and the tooltip.
pub(crate) fn drop_button(ui: &mut egui::Ui, label: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(14.0), egui::Sense::click());
    let colour = if response.hovered() { crate::theme::token::DANGER } else { crate::theme::token::TEXT_LO };
    let arm = rect.shrink(3.5);
    let stroke = egui::Stroke::new(1.4_f32, colour);
    ui.painter().line_segment([arm.left_top(), arm.right_bottom()], stroke);
    ui.painter().line_segment([arm.right_top(), arm.left_bottom()], stroke);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response.on_hover_text(label).clicked()
}

/// A field meaningful only while its checkbox is ticked, greyed rather than hidden so the layout
/// stays put and the value stays visible.
pub(crate) fn optional(
    app: &mut App,
    ui: &mut egui::Ui,
    on: &mut bool,
    field: (&str, &str),
    width: f32,
    kind: ParamKind,
    value: &mut f64,
) {
    ui.checkbox(on, "");
    let enabled = *on;
    ui.add_enabled_ui(enabled, |ui| number(app, ui, field, width, kind, value));
}
