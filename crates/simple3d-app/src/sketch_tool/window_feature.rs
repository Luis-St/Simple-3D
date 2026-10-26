//! The panel's feature section: what the picked regions become, how far, and whether the solid
//! adds to the model or is cut out of it, in the properties panel's rows. The same parameters stay
//! editable in the properties panel once the sketch is kept.

use super::window::group;
use super::*;
use crate::app::App;
use crate::panel_properties::{field_row, field_row_boxed, named, room_left};
use crate::{popup, theme};
use simple3d_core::primitive::{ParamKind, ParamValue, ParamsExt};
use simple3d_core::scene::{sketch_param_shown, EXTENTS, FEATURES, SKETCH_PARAMS};

/// Chips choosing one of `options` for parameter `key`.
fn choice(ui: &mut egui::Ui, tool: &mut SketchTool, key: &str, options: &[&str]) {
    let current = tool.params.int(key);
    for (index, option) in options.iter().enumerate() {
        if theme::choice(ui, current == index as u32, option).clicked() {
            tool.params.insert(key.into(), ParamValue::Choice(index as u32));
        }
    }
}

pub(crate) fn feature(app: &mut App, ui: &mut egui::Ui, tool: &mut SketchTool) {
    group(ui, "Make", |ui| make(app, ui, tool));
}

fn make(app: &mut App, ui: &mut egui::Ui, tool: &mut SketchTool) {
    let revolve = tool.params.int("feature") == 1;
    field_row_boxed(ui, "Feature", "What the picked regions become.", |ui| choice(ui, tool, "feature", FEATURES));
    if !revolve {
        field_row_boxed(ui, "Extent", "Which way the solid runs from the sketch plane.", |ui| {
            choice(ui, tool, "extent", EXTENTS)
        });
    }
    let unit = app.unit().suffix();
    for spec in
        SKETCH_PARAMS.iter().filter(|spec| matches!(spec.kind, ParamKind::Length { .. } | ParamKind::Angle { .. }))
    {
        if !sketch_param_shown(spec.key, &tool.params) {
            continue;
        }
        let length = matches!(spec.kind, ParamKind::Length { .. });
        let label = named(spec.label, if length { unit } else { "deg" });
        field_row(ui, &label, hover(spec.key), |ui| {
            let mut value = tool.params.num(spec.key);
            let width = room_left(ui).max(40.0);
            popup::number(app, ui, ("sketch-feature", spec.key), width, spec.kind, &mut value);
            let stored = if length { ParamValue::Length(value) } else { ParamValue::Angle(value) };
            tool.params.insert(spec.key.into(), stored);
        });
    }
    let reverse = if revolve { "Turn the other way" } else { "Run the other way" };
    field_row(ui, "Direction", "Flip the solid to the other side of the sketch plane.", |ui| {
        let mut flip = tool.params.flag("flip");
        if theme::toggle(ui, &mut flip, reverse).changed() {
            tool.params.insert("flip".into(), ParamValue::Bool(flip));
        }
    });
    if revolve {
        let axis = match tool.sketch.axis {
            Some(_) => "Revolves about the dashed line; the Revolve axis tool picks another.",
            None => "Pick the line to revolve about with the Revolve axis tool.",
        };
        field_row_boxed(ui, "Axis", "", |ui| {
            ui.add(egui::Label::new(theme::hint(axis)).selectable(false).wrap());
        });
    }
    if tool.editing.is_none() {
        field_row_boxed(ui, "Result", "Whether the solid adds to the model or is cut out of it.", |ui| {
            for (placing, label, hover) in [
                (Placing::Add, "Add", "The solid adds to the model."),
                (Placing::Cut, "Cut", "The solid is cut out of the object the sketch is on, or out of everything."),
            ] {
                if theme::choice(ui, tool.placing == placing, label).on_hover_text(hover).clicked() {
                    tool.placing = placing;
                }
            }
        });
    }
}

fn hover(key: &str) -> &'static str {
    match key {
        "distance" => "How far the solid runs from the sketch plane; in all, when symmetric.",
        "distance2" => "How far it runs the other way.",
        "draft" => "How far the walls lean in as they leave the sketch plane; below zero they lean out.",
        "sweep" => "How far round the axis the regions are turned.",
        _ => "",
    }
}
