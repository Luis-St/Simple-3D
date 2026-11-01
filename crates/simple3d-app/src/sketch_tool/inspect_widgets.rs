//! The popup's widgets: a named number that a fix can lock, the lock beside a size, and the switches.

use crate::app::App;
use crate::popup;
use crate::theme::token;
use simple3d_core::primitive::ParamKind;

const FIELD_WIDTH: f32 = 110.0;
/// What a locked field says on hover.
const LOCKED_HOVER: &str = "Fixed, so it stays where it is; switch Fixed off to change it";

/// A named number; the new value when it was changed this frame. A `locked` one, held by a fix, is
/// drawn greyed with a padlock beside its name and takes no edits.
pub(super) fn number(
    app: &mut App,
    ui: &mut egui::Ui,
    (field, label): (&str, &str),
    kind: ParamKind,
    value: f64,
    locked: bool,
) -> Option<f64> {
    // The padlock goes beside the name, so the number stays readable.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        popup::label(ui, label, if locked { LOCKED_HOVER } else { "" });
        if locked {
            let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(11.0), egui::Sense::hover());
            padlock(ui.painter(), rect, token::TEXT_LO);
            response.on_hover_text(LOCKED_HOVER);
        }
    });
    let mut shown = value;
    ui.add_enabled_ui(!locked, |ui| {
        popup::number(app, ui, ("sketch-inspect", field), FIELD_WIDTH, kind, &mut shown);
    })
    .response
    .on_disabled_hover_text(LOCKED_HOVER);
    if locked {
        return None;
    }
    ((shown - value).abs() > 1e-9).then_some(shown)
}

/// A closed padlock filling `rect`: a shackle over a solid body. Painted, since the UI font has no
/// glyph for it.
fn padlock(painter: &egui::Painter, rect: egui::Rect, colour: egui::Color32) {
    let body = egui::Rect::from_min_max(egui::pos2(rect.left(), rect.top() + rect.height() * 0.45), rect.max);
    painter.rect_filled(body, 1.0, colour);
    let radius = rect.width() * 0.3;
    let centre = egui::pos2(rect.center().x, rect.top() + radius + 0.5);
    let shackle: Vec<egui::Pos2> = (0..=12)
        .map(|k| {
            let turn = std::f32::consts::PI * (k as f32 / 12.0);
            centre + egui::vec2(-radius * turn.cos(), -radius * turn.sin())
        })
        .collect();
    let mut points = vec![egui::pos2(centre.x - radius, body.top())];
    points.extend(shackle);
    points.push(egui::pos2(centre.x + radius, body.top()));
    painter.add(egui::Shape::line(points, egui::Stroke::new(1.5_f32, colour)));
}

/// The lock beside a size: lit while a dimension holds it. Whether it was clicked.
pub(super) fn lock(ui: &mut egui::Ui, locked: bool) -> bool {
    let hover = if locked { "Held by a dimension; click to free it" } else { "Hold this size with a dimension" };
    ui.add(egui::Button::selectable(locked, "Lock")).on_hover_text(hover).clicked()
}

/// A switch in the row under the fields; whether it was clicked.
pub(super) fn switch(ui: &mut egui::Ui, on: bool, label: &str, hover: &str) -> bool {
    ui.add(egui::Button::selectable(on, label)).on_hover_text(hover).clicked()
}
