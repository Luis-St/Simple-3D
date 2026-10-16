//! The tool's settings, one aligned row each: the objects, the line they are mirrored across, the
//! plane through it, and what Apply makes.

use super::*;
use crate::app::App;
use crate::panel_properties::{point_fields, scalar_field, Scalar, POINT};
use crate::{popup, theme};

const AXES: [&str; 3] = ["X", "Y", "Z"];

/// Every row in one grid, so the names line up in a column as in the align tool. Returns whether
/// the objects' Change or Done was pressed, which the caller carries out with the tool back in place.
pub(super) fn rows(app: &mut App, ui: &mut egui::Ui, tool: &mut MirrorTool) -> bool {
    egui::Grid::new("mirror-tool")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            let toggled = objects_row(app, ui, tool);
            across_row(ui, tool);
            match tool.by {
                MirrorBy::Axis => axis_row(ui, tool),
                MirrorBy::Edge | MirrorBy::Line => end_rows(app, ui, tool),
            }
            plane_row(ui, tool);
            result_row(ui, tool);
            toggled
        })
        .inner
}

/// What is mirrored. Across an axis that is simply the selection; otherwise a chip hands the
/// viewport back to the selection to change it, since its clicks pick the line meanwhile.
fn objects_row(app: &App, ui: &mut egui::Ui, tool: &MirrorTool) -> bool {
    popup::label(ui, "Objects", "What is mirrored.");
    let toggled = ui
        .horizontal(|ui| {
            let count = tool.targets.len();
            let names = match tool.targets[..] {
                [] => "Nothing selected".to_string(),
                [only] => app.scene.get(only).map_or_else(String::new, |node| node.name.clone()),
                _ => format!("{count} object{}", crate::ui::plural(count)),
            };
            let text = if count == 0 { theme::hint(names) } else { theme::value(names) };
            ui.add(egui::Label::new(text).truncate().selectable(false));
            if tool.by == MirrorBy::Axis {
                return false;
            }
            let (chip, hover) = match tool.choosing {
                true => ("Done", "Keep the selected objects; the viewport's clicks then pick the line again."),
                false => ("Change", "Choose other objects by selecting them as usual."),
            };
            let enabled = !tool.choosing || count > 0;
            ui.add_enabled_ui(enabled, |ui| theme::choice(ui, tool.choosing, chip)).inner.on_hover_text(hover).clicked()
        })
        .inner;
    ui.end_row();
    toggled
}

/// What the objects are mirrored across.
fn across_row(ui: &mut egui::Ui, tool: &mut MirrorTool) {
    popup::label(ui, "Across", "What the objects are mirrored across.");
    ui.horizontal(|ui| {
        for (by, hover) in [
            (MirrorBy::Axis, "One of the world axes, through the origin."),
            (MirrorBy::Edge, "A body edge clicked in the viewport; a right-click lets it go."),
            (
                MirrorBy::Line,
                "A line drawn with two clicks in the viewport, catching corners, edges and faces; its ends can \
                 be typed. A right-click starts again.",
            ),
        ] {
            if theme::choice(ui, tool.by == by, by.label()).on_hover_text(hover).clicked() && tool.by != by {
                tool.by = by;
                tool.ends = [None, None];
            }
        }
    });
    ui.end_row();
}

fn axis_row(ui: &mut egui::Ui, tool: &mut MirrorTool) {
    popup::label(ui, "Axis", "The world axis the objects are mirrored across.");
    ui.horizontal(|ui| {
        for (axis, name) in AXES.into_iter().enumerate() {
            if theme::choice(ui, tool.axis == axis, name).clicked() {
                tool.axis = axis;
            }
        }
    });
    ui.end_row();
}

/// The picked edge's or drawn line's ends as fields, once there are any; until then the summary
/// says what to click.
fn end_rows(app: &mut App, ui: &mut egui::Ui, tool: &mut MirrorTool) {
    let unit = app.unit();
    let step = unit.from_mm(app.move_snap()).max(1e-6);
    for (index, name) in ["Start", "End"].into_iter().enumerate() {
        // An edge has both ends or none; a drawn line shows its end once started.
        let Some(at) = tool.ends[index].or(tool.ends[0].filter(|_| tool.by == MirrorBy::Line)) else { continue };
        popup::label(ui, name, &format!("Where the line {}s, in {}.", name.to_lowercase(), unit.suffix()));
        ui.horizontal(|ui| {
            point_fields(ui, name, |ui, axis| {
                let grip = format!("mirror-end-{index}:{axis}");
                let id = ui.id().with(("mirror-end", index, axis));
                // No undo step: the line belongs to the tool, not the scene.
                let current = at.get(axis);
                scalar_field(app, ui, Scalar { grip: &grip, id, kind: POINT, current, step }, |_, mm, _| {
                    let mut point = at;
                    point.set(axis, mm);
                    tool.ends[index] = Some(point);
                });
            });
        });
        ui.end_row();
    }
}

/// The world direction the plane holds beside the line.
fn plane_row(ui: &mut egui::Ui, tool: &mut MirrorTool) {
    popup::label(
        ui,
        "Plane along",
        "A line alone does not fix the mirror: its plane holds the line and this direction too. Z keeps it \
         upright, so the objects flip over the line as seen from above.",
    );
    ui.horizontal(|ui| {
        let along = tool.line().ok().map(|(_, along)| along);
        for (axis, name) in AXES.into_iter().enumerate() {
            // The line's own direction adds nothing to it.
            let fits = along.is_none_or(|along| along.get(axis).abs() < PARALLEL);
            let response = ui.add_enabled_ui(fits, |ui| theme::choice(ui, tool.holds() == axis, name)).inner;
            if response.clicked() {
                tool.upright = axis;
            }
        }
    });
    ui.end_row();
}

/// Copies beside the originals, or the originals themselves.
fn result_row(ui: &mut egui::Ui, tool: &mut MirrorTool) {
    popup::label(ui, "Make", "What Apply leaves behind.");
    ui.horizontal(|ui| {
        for (keep, name, hover) in [
            (true, "Copies", "Make mirrored copies beside the objects and keep the originals."),
            (false, "Move originals", "Mirror the objects themselves; nothing is copied."),
        ] {
            if theme::choice(ui, tool.keep == keep, name).on_hover_text(hover).clicked() {
                tool.keep = keep;
            }
        }
    });
    ui.end_row();
}
