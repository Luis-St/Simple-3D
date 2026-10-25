//! The window's feature section: what the picked regions become, how far, and whether the solid
//! adds to the model or is cut out of it. The same parameters stay editable in the properties panel
//! once the sketch is kept.

use super::window::heading;
use super::*;
use crate::app::App;
use crate::popup;
use simple3d_core::primitive::{ParamKind, ParamValue, ParamsExt};
use simple3d_core::scene::{sketch_param_shown, EXTENTS, FEATURES, SKETCH_PARAMS};

const FIELD_WIDTH: f32 = 90.0;

/// Buttons choosing one of `options` for parameter `key`.
fn choice(ui: &mut egui::Ui, tool: &mut SketchTool, key: &str, options: &[&str]) {
    let current = tool.params.int(key);
    ui.horizontal(|ui| {
        for (index, option) in options.iter().enumerate() {
            if ui.add(egui::Button::selectable(current == index as u32, *option)).clicked() {
                tool.params.insert(key.into(), ParamValue::Choice(index as u32));
            }
        }
    });
}

pub(crate) fn feature(app: &mut App, ui: &mut egui::Ui, tool: &mut SketchTool) {
    heading(ui, "Make");
    choice(ui, tool, "feature", FEATURES);
    if tool.params.int("feature") == 0 {
        choice(ui, tool, "extent", EXTENTS);
    }
    let unit = app.unit().suffix();
    egui::Grid::new("sketch-feature").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
        for spec in
            SKETCH_PARAMS.iter().filter(|spec| matches!(spec.kind, ParamKind::Length { .. } | ParamKind::Angle { .. }))
        {
            if !sketch_param_shown(spec.key, &tool.params) {
                continue;
            }
            let suffix = if matches!(spec.kind, ParamKind::Length { .. }) { unit } else { "deg" };
            popup::label(ui, &format!("{} ({suffix})", spec.label), hover(spec.key));
            let mut value = tool.params.num(spec.key);
            popup::number(app, ui, ("sketch-feature", spec.key), FIELD_WIDTH, spec.kind, &mut value);
            let stored = match spec.kind {
                ParamKind::Length { .. } => ParamValue::Length(value),
                _ => ParamValue::Angle(value),
            };
            tool.params.insert(spec.key.into(), stored);
            ui.end_row();
        }
    });
    let mut flip = tool.params.flag("flip");
    let reverse = if tool.params.int("feature") == 1 { "Turn the other way" } else { "Run the other way" };
    if ui.checkbox(&mut flip, reverse).changed() {
        tool.params.insert("flip".into(), ParamValue::Bool(flip));
    }
    if tool.params.int("feature") == 1 {
        let axis = match tool.sketch.axis {
            Some(_) => "Revolves about the dashed line; Revolve axis picks another.",
            None => "Pick the line to revolve about with Revolve axis.",
        };
        ui.add(egui::Label::new(crate::theme::hint(axis)).selectable(false));
    }
    if tool.editing.is_none() {
        ui.horizontal(|ui| {
            for (placing, label, hover) in [
                (Placing::Add, "Add", "The solid adds to the model."),
                (Placing::Cut, "Cut", "The solid is cut out of the object the sketch is on, or out of everything."),
            ] {
                if ui.add(egui::Button::selectable(tool.placing == placing, label)).on_hover_text(hover).clicked() {
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
