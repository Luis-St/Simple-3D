//! The panel's tools: the pointer, which only selects, then a row of tiles each for drawing,
//! modifying, constraining and picking -- one tile per family of tools, showing the variant it picks, the one in
//! use lit -- with the variants of the family in use straight under its row, and under them all the
//! settings of the tool in use: the fillet's radius, the polygon's side count, the pattern's count
//! and so on.

use super::settings::{MAX_COPIES, MAX_SIDES};
use super::*;
use crate::app::App;
use crate::panel_properties::{field_row, field_row_boxed, named, room_left};
use crate::{icon, popup, theme};
use simple3d_core::primitive::ParamKind;

/// A tool tile's side, and a variant tile's.
const TILE: f32 = 30.0;
const VARIANT_TILE: f32 = 24.0;
const LENGTH: ParamKind = ParamKind::Length { min: 0.0 };
const SIDES: ParamKind = ParamKind::Count { min: 3, max: MAX_SIDES };
const COPIES: ParamKind = ParamKind::Count { min: 2, max: MAX_COPIES };
const SPREAD: ParamKind = ParamKind::Angle { min: 1.0, max: 360.0, wrap: false };

pub(crate) fn tools(app: &mut App, ui: &mut egui::Ui, tool: &mut SketchTool) {
    super::window::group(ui, "Tools", |ui| {
        field_row(ui, "Pointer", "Select and change what is drawn without adding to it.", |ui| {
            pointer_tile(ui, tool);
            ui.add_space(6.0);
            ui.add(egui::Label::new(theme::hint("Select and drag, never draws")).selectable(false));
        });
        for (name, hover, families) in [
            ("Draw", "Put new lines, curves and points down.", &Draw::DRAWING[..]),
            ("Modify", "Cut, round, copy and move what is drawn.", &Draw::MODIFY[..]),
            (
                "Constrain",
                "Dimension what is drawn and hold it in shape; with what a constraint needs selected, its tile puts \
                 it straight on.",
                &Draw::CONSTRAIN[..],
            ),
            ("Pick", "Pick what the solid is made from, or copy edges in from the model.", &Draw::PICKING[..]),
        ] {
            let mut said = None;
            field_row_boxed(ui, name, hover, |ui| said = tiles(ui, tool, families));
            // The kinds go straight under the row their family's tile is in.
            if families.contains(&tool.draw.family()) {
                said = variants(ui, tool).or(said);
            }
            if let Some(status) = said {
                app.status = status;
            }
        }
        settings(app, ui, tool);
    });
}

fn pointer_tile(ui: &mut egui::Ui, tool: &mut SketchTool) {
    let response = icon::button(ui, Draw::Select.glyph(), TILE, tool.draw == Draw::Select, true);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, Draw::Select.label()));
    if response.on_hover_text(format!("{}\n{}", Draw::Select.named(), Draw::Select.hint())).clicked() {
        tool.set_draw(Draw::Select);
    }
}

/// One tile per family, showing the variant it picks; the one in use filled with the accent, its
/// name and key in the tooltip. What choosing one said, if anything.
fn tiles(ui: &mut egui::Ui, tool: &mut SketchTool, families: &[Family]) -> Option<crate::app::Status> {
    ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
    let mut said = None;
    for &family in families {
        let draw = if family.contains(&tool.draw) { tool.draw } else { tool.settings.variant(family) };
        let response = icon::button(ui, draw.glyph(), TILE, family.contains(&tool.draw), true);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, draw.label()));
        let more = if family.len() > 1 { "\nThe kinds of it are shown under the tools." } else { "" };
        if response.on_hover_text(format!("{}\n{}{more}", draw.named(), draw.hint())).clicked() {
            said = tool.choose(draw);
        }
    }
    said
}

/// The variants of the family in use, as smaller tiles, with the one in use named beside them.
fn variants(ui: &mut egui::Ui, tool: &mut SketchTool) -> Option<crate::app::Status> {
    let family = tool.draw.family();
    if family.len() < 2 {
        return None;
    }
    let mut said = None;
    field_row(ui, "Kind", "Which kind of this tool to use; its key steps through them.", |ui| {
        ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
        for &draw in family {
            let response = icon::button(ui, draw.glyph(), VARIANT_TILE, tool.draw == draw, true);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, draw.label()));
            if response.on_hover_text(format!("{}\n{}", draw.label(), draw.hint())).clicked() {
                said = tool.choose(draw);
            }
        }
        ui.add_space(6.0);
        ui.add(egui::Label::new(theme::hint(tool.draw.label())).selectable(false));
    });
    said
}

/// A length setting's row.
fn length_row(app: &mut App, ui: &mut egui::Ui, name: &str, hover: &str, key: &str, value: &mut f64) {
    let label = named(name, app.unit().suffix());
    field_row(ui, &label, hover, |ui| {
        let width = room_left(ui).max(40.0);
        popup::number(app, ui, ("sketch-tool", key), width, LENGTH, value);
    });
}

/// A whole-number setting's row.
fn count_row(
    app: &mut App,
    ui: &mut egui::Ui,
    (name, hover, key): (&str, &str, &str),
    kind: ParamKind,
    value: &mut u32,
) {
    field_row(ui, name, hover, |ui| {
        let mut v = *value as f64;
        let width = room_left(ui).max(40.0);
        popup::number(app, ui, ("sketch-tool", key), width, kind, &mut v);
        *value = v.round().max(0.0) as u32;
    });
}

/// The settings of the tool in use, if it has any.
fn settings(app: &mut App, ui: &mut egui::Ui, tool: &mut SketchTool) {
    let s = &mut tool.settings;
    match tool.draw {
        Draw::Fillet | Draw::RoundedRectangle => {
            length_row(
                app,
                ui,
                "Radius",
                "The radius corners are rounded with.",
                "fillet-radius",
                &mut s.fillet_radius,
            );
        }
        Draw::Chamfer => {
            let hover = "How far along both lines the corner is cut off.";
            length_row(app, ui, "Distance", hover, "chamfer-distance", &mut s.chamfer_distance);
        }
        Draw::Offset => {
            let hover = "How far to the side the copy goes.";
            length_row(app, ui, "Distance", hover, "offset-distance", &mut s.offset_distance);
        }
        Draw::Polygon | Draw::CircumscribedPolygon => {
            count_row(
                app,
                ui,
                ("Sides", "How many sides the polygon has.", "polygon-sides"),
                SIDES,
                &mut s.polygon_sides,
            );
        }
        Draw::LinearPattern | Draw::CircularPattern => {
            let hover = "How many there are, the original counted.";
            count_row(app, ui, ("Count", hover, "pattern-count"), COPIES, &mut s.pattern_count);
            if tool.draw == Draw::CircularPattern {
                let label = named("Spread", "\u{00B0}");
                let hover = "How far round the copies spread; a full turn spaces them evenly all round.";
                field_row(ui, &label, hover, |ui| {
                    let width = room_left(ui).max(40.0);
                    popup::number(app, ui, ("sketch-tool", "pattern-angle"), width, SPREAD, &mut s.pattern_angle);
                });
            }
        }
        Draw::CopyEdges => {
            let hover = "Copied edges are fixed either way; as helper geometry they bound no region.";
            field_row(ui, "Copies", hover, |ui| {
                theme::toggle(ui, &mut s.copy_as_helper, "As helper geometry");
            });
        }
        _ => {}
    }
    if tool.draw.draws() && tool.draw != Draw::Point {
        let hover = "Helper geometry is constrained against and can be an axis, but bounds no region.";
        field_row(ui, "Draws", hover, |ui| {
            let text = format!("Helper geometry ({})", super::keys::CONSTRUCTION_KEY.name());
            theme::toggle(ui, &mut tool.settings.construction, &text);
        });
    }
    if tool.draw.uses_selection() {
        let curves = tool.selection.iter().filter(|p| matches!(p, Pick::Curve(_))).count();
        let text = match curves {
            0 => "Nothing selected yet".to_string(),
            1 => "1 curve selected".to_string(),
            n => format!("{n} curves selected"),
        };
        field_row(ui, "Selection", "What the tool works on; Shift-click curves to add or drop them.", |ui| {
            ui.add(egui::Label::new(theme::hint(text)).selectable(false));
        });
    }
}
