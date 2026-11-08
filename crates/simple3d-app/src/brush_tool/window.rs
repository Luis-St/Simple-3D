//! The brush's popup: its mode, size, strength and falloff, adding detail, and what it keeps.

use super::*;
use crate::popup::{self, label, PopupEvent, PopupSpec};
use crate::theme;
use crate::ui;
use simple3d_core::keymap::Command;
use simple3d_core::primitive::ParamKind;
use simple3d_geom::brush::{Brush, Falloff, Mode};

/// Scopes the window's fields, so a drag passed between tool windows cannot edit the wrong number.
const SCOPE: &str = "brush-field";

/// The window width: a label and its field, and the five modes in a row.
const WIDTH: f32 = 360.0;
const FIELD_WIDTH: f32 = 90.0;

const RADIUS: ParamKind = ParamKind::Length { min: Brush::RADIUS.0 };
const STRENGTH: ParamKind = ParamKind::Count { min: 1, max: 100 };
const EDGE: ParamKind = ParamKind::Length { min: 0.05 };
const SCALE: ParamKind = ParamKind::Length { min: 0.01 };
const SEED: ParamKind = ParamKind::Count { min: 1, max: 99_999 };
const BASE: ParamKind = ParamKind::Length { min: 0.0 };

/// The window over the viewport, while the brush is on the rail.
pub(crate) fn show(app: &mut App, ctx: &egui::Context) {
    if app.mode != crate::gizmo::Mode::Brush {
        return;
    }
    let spec = PopupSpec { key: KEY, title: "Brush", width: WIDTH };
    let event = app.tool_popup(ctx, spec, body, actions);
    if event == PopupEvent::Closed {
        app.run(Command::ModeMove);
    }
}

fn body(app: &mut App, ui: &mut egui::Ui) {
    let before = app.settings.last_brush;
    let mut brush = before;
    let length = format!("({})", app.unit().suffix());
    let number = |app: &mut App, ui: &mut egui::Ui, name: &str, kind: ParamKind, value: &mut f64| {
        popup::number(app, ui, (SCOPE, name), FIELD_WIDTH, kind, value);
    };
    // The modes in a row of their own above the numbers: wrapped inside the grid's narrow column, the
    // chips stretched every row after them.
    ui.horizontal(|ui| {
        popup::label(ui, "Mode", "What a dab does. Ctrl inverts raise and lower while held; Shift smooths.");
        for mode in Mode::ALL {
            if theme::choice(ui, brush.mode == mode, mode.label()).on_hover_text(mode_hint(mode)).clicked() {
                brush.mode = mode;
            }
        }
    });
    ui.add_space(6.0);
    egui::Grid::new("brush-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        let keys = format!(
            "{} / {}",
            app.keymap.shortcut_text(Command::BrushSmaller),
            app.keymap.shortcut_text(Command::BrushLarger)
        );
        label(ui, &format!("Radius {length}"), &format!("How far a dab reaches. {keys}, or Ctrl and the wheel."));
        number(app, ui, "radius", RADIUS, &mut brush.radius);
        ui.end_row();

        let keys = format!(
            "{} / {}",
            app.keymap.shortcut_text(Command::BrushWeaker),
            app.keymap.shortcut_text(Command::BrushStronger)
        );
        label(
            ui,
            "Strength (%)",
            &format!("How much one dab does. At full strength a raise lifts a tenth of the radius. {keys}, or Ctrl+Shift and the wheel."),
        );
        let mut percent = (brush.strength * 100.0).round();
        number(app, ui, "strength", STRENGTH, &mut percent);
        brush.strength = (percent / 100.0).clamp(Brush::STRENGTH.0, Brush::STRENGTH.1);
        ui.end_row();

        label(ui, "Falloff", "How a dab fades from its centre to its edge.");
        ui.horizontal(|ui| {
            for falloff in Falloff::ALL {
                if theme::choice(ui, brush.falloff == falloff, falloff.label()).clicked() {
                    brush.falloff = falloff;
                }
            }
        });
        ui.end_row();

        if brush.mode == Mode::Noise {
            label(ui, &format!("Size {length}"), "How big the noise's bumps are.");
            number(app, ui, "scale", SCALE, &mut brush.scale);
            ui.end_row();
            label(ui, "Seed", "Which of the endless noise patterns. The same seed always gives the same one.");
            let mut seed = f64::from(brush.seed);
            number(app, ui, "seed", SEED, &mut seed);
            brush.seed = seed.round().max(1.0) as u32;
            ui.end_row();
        }

        label(
            ui,
            &format!("Add detail {length}"),
            "Split the triangles under each dab until no edge is longer than this, so a coarse mesh has the \
             corners to take the stroke. Only where the brush goes. Off, a dab still splits edges longer than half \
             its radius, so even a converted box has something to move.",
        );
        ui.horizontal(|ui| {
            popup::optional(app, ui, &mut brush.detail, (SCOPE, "edge"), FIELD_WIDTH, EDGE, &mut brush.edge);
        });
        ui.end_row();

        label(
            ui,
            &format!("Keep the base {length}"),
            "Leave the bottom of the mesh as it is up to this height, fading in above it, so a print still \
             sits flat.",
        );
        ui.horizontal(|ui| {
            popup::optional(app, ui, &mut brush.keep_base, (SCOPE, "base"), FIELD_WIDTH, BASE, &mut brush.base);
        });
        ui.end_row();

        label(ui, "Keep open edges", "Leave the rim of a hole exactly where it is.");
        ui.checkbox(&mut brush.keep_boundaries, "");
        ui.end_row();
    });
    if app.brush.over.or_else(|| target(app)).is_none() {
        ui.add_space(6.0);
        ui.label(theme::hint("Point at a mesh body to brush it."));
    }
    if brush != before {
        app.settings.last_brush = brush;
        app.persist();
    }
}

fn actions(app: &mut App, ui: &mut egui::Ui) {
    if ui::dialog_button(ui, "Done", true).clicked() {
        app.run(Command::ModeMove);
    }
}

fn mode_hint(mode: Mode) -> &'static str {
    match mode {
        Mode::Raise => "Lift the surface out",
        Mode::Lower => "Press the surface in",
        Mode::Smooth => "Take out bumps without shrinking the shape",
        Mode::Flatten => "Pull the surface towards the plane under the brush",
        Mode::Noise => "Roughen the surface with a noise pattern",
    }
}
