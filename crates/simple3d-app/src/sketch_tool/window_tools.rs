//! The panel's tools: the pointer, which only selects, then a row of tiles each for drawing,
//! modifying and picking, the one in use lit, and under them the settings of the tool in use -- the
//! fillet's radius, the polygon's side count, whether copied edges are helper geometry -- each shown
//! while its tool is picked.

use super::*;
use crate::app::App;
use crate::panel_properties::{field_row, field_row_boxed, named, room_left};
use crate::{icon, popup, theme};
use simple3d_core::primitive::ParamKind;

/// A tool tile's side.
const TILE: f32 = 30.0;
const RADIUS: ParamKind = ParamKind::Length { min: 0.0 };
const SIDES: ParamKind = ParamKind::Count { min: 3, max: super::drawing::MAX_SIDES };

pub(crate) fn tools(app: &mut App, ui: &mut egui::Ui, tool: &mut SketchTool) {
    super::window::group(ui, "Tools", |ui| {
        field_row(ui, "Pointer", "Select and change what is drawn without adding to it.", |ui| {
            tiles(ui, tool, &[Draw::Select]);
            ui.add_space(6.0);
            ui.add(egui::Label::new(theme::hint("Select and drag, never draws")).selectable(false));
        });
        for (name, hover, tools) in [
            ("Draw", "Put new lines, curves and points down.", &Draw::DRAWING[..]),
            ("Modify", "Cut away, round off and dimension what is drawn.", &Draw::MODIFY[..]),
            ("Pick", "Pick what the solid is made from, or copy edges in from the model.", &Draw::PICKING[..]),
        ] {
            field_row_boxed(ui, name, hover, |ui| tiles(ui, tool, tools));
        }
        settings(app, ui, tool);
    });
}

/// One tile per tool, the one in use filled with the accent, its name and key in the tooltip.
fn tiles(ui: &mut egui::Ui, tool: &mut SketchTool, tools: &[Draw]) {
    ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
    for &draw in tools {
        let response = icon::button(ui, draw.glyph(), TILE, tool.draw == draw, true);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, draw.label()));
        if response.on_hover_text(format!("{}\n{}", draw.named(), draw.hint())).clicked() {
            tool.set_draw(draw);
        }
    }
}

/// The settings of the tool in use, if it has any.
fn settings(app: &mut App, ui: &mut egui::Ui, tool: &mut SketchTool) {
    match tool.draw {
        Draw::Fillet => {
            let label = named("Radius", app.unit().suffix());
            field_row(ui, &label, "The radius corners are rounded with.", |ui| {
                let width = room_left(ui).max(40.0);
                popup::number(app, ui, ("sketch-tool", "fillet-radius"), width, RADIUS, &mut tool.fillet_radius);
            });
        }
        Draw::Polygon => {
            field_row(ui, "Sides", "How many sides the polygon has.", |ui| {
                let mut sides = tool.polygon_sides as f64;
                let width = room_left(ui).max(40.0);
                popup::number(app, ui, ("sketch-tool", "polygon-sides"), width, SIDES, &mut sides);
                tool.polygon_sides = (sides.round() as u32).clamp(3, super::drawing::MAX_SIDES);
            });
        }
        Draw::CopyEdges => {
            let hover = "Copied edges are fixed either way; as helper geometry they bound no region.";
            field_row(ui, "Copies", hover, |ui| {
                theme::toggle(ui, &mut tool.copy_as_helper, "As helper geometry");
            });
        }
        _ => {}
    }
}
