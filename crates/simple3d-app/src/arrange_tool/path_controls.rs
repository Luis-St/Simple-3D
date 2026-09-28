//! The settings for spreading along a path: where the path comes from, and how many go on it.

use super::*;
use crate::app::App;
use crate::panel_properties::{point_fields, scalar_field, Scalar, POINT};
use crate::{popup, theme};
use simple3d_core::primitive::ParamKind;

pub(super) fn path_controls(app: &mut App, ui: &mut egui::Ui, tool: &mut ArrangeTool) {
    ui.horizontal_wrapped(|ui| {
        popup::label(ui, "Path", "Where the path comes from.");
        for (source, name, hover) in [
            (
                PathSource::Edges,
                "Pick edges",
                "Click the edges of bodies in the viewport; they are joined in the order they meet.",
            ),
            (PathSource::Drawn, "Draw", "Click points in the viewport; they are joined by straight lines."),
        ] {
            if theme::choice(ui, tool.source == source, name).on_hover_text(hover).clicked() {
                tool.source = source;
            }
        }
    });
    ui.add_space(4.0);
    match tool.source {
        PathSource::Edges => edge_rows(app, ui, tool),
        PathSource::Drawn => point_rows(app, ui, tool),
    }
    ui.add_space(6.0);

    egui::Grid::new("arrange-path").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        let selected = tool.targets.len();
        popup::label(
            ui,
            "Count",
            "How many objects go along the path, evenly spaced. More than are selected, and the rest are \
             copies, repeating the selection in order.",
        );
        let mut count = tool.count.max(selected) as f64;
        let kind = ParamKind::Count { min: selected as u32, max: MOST_COUNT.max(selected as u32) };
        number(app, ui, "count", kind, &mut count);
        tool.count = count.round().max(selected as f64) as usize;
        ui.end_row();

        popup::label(ui, "Turn", "Turn each object about Z as the path turns; the first keeps the turn it has.");
        ui.checkbox(&mut tool.follow, "With the path");
        ui.end_row();
    });
}

fn edge_rows(app: &mut App, ui: &mut egui::Ui, tool: &mut ArrangeTool) {
    ui.add(
        egui::Label::new(theme::hint(
            "Click body edges in the viewport. A picked edge clicked again is taken out; a right-click takes \
             the last click back.",
        ))
        .selectable(false),
    );
    ui.checkbox(&mut tool.whole_run, "Take the whole smooth run").on_hover_text(
        "A click takes the edge and every edge running on from it without a sharp corner: a round rim \
         whole, a box edge on its own.",
    );
    let edges: usize = tool.runs.iter().map(Vec::len).sum();
    ui.horizontal(|ui| {
        let length = simple3d_core::unit::format_length(tool.path().length(), app.unit());
        ui.label(format!("{edges} edge{}, {length} {} long", crate::ui::plural(edges), app.unit().suffix()));
        if ui.add_enabled(edges > 0, egui::Button::new("Clear")).clicked() {
            tool.runs.clear();
        }
    });
}

fn point_rows(app: &mut App, ui: &mut egui::Ui, tool: &mut ArrangeTool) {
    ui.add(
        egui::Label::new(theme::hint(
            "Click points in the viewport; they catch corners, edges and faces like the measure tool. A \
             right-click takes the last one back.",
        ))
        .selectable(false),
    );
    let unit = app.unit();
    let step = unit.from_mm(app.move_snap()).max(1e-6);
    let mut dropped = None;
    for index in 0..tool.points.len() {
        ui.horizontal(|ui| {
            // The cross before the fields, which take the rest of the row.
            if popup::drop_button(ui, &format!("Drop point {}", index + 1)) {
                dropped = Some(index);
            }
            ui.label(format!("{}", index + 1));
            let name = format!("Point {}", index + 1);
            point_fields(ui, &name, |ui, axis| {
                let grip = format!("arrange-point-{index}:{axis}");
                let id = ui.id().with(("arrange-point", index, axis));
                let current = tool.points[index].get(axis);
                // No undo step: the path belongs to the tool, not the scene.
                scalar_field(app, ui, Scalar { grip: &grip, id, kind: POINT, current, step }, |_, mm, _| {
                    tool.points[index].set(axis, mm);
                });
            });
        });
    }
    if let Some(index) = dropped {
        tool.points.remove(index);
    }
    ui.horizontal(|ui| {
        ui.add_enabled_ui(tool.points.len() > 2, |ui| ui.checkbox(&mut tool.closed, "Back to the start"))
            .response
            .on_hover_text("Join the last point to the first, so the objects go all the way round.");
        if ui.add_enabled(!tool.points.is_empty(), egui::Button::new("Clear")).clicked() {
            tool.points.clear();
        }
    });
}
