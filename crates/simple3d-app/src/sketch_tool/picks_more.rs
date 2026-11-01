//! The clicks of the newer modifying tools: extend, split, chamfer and offset on what is under the
//! pointer, and the selection tools -- mirror, patterns, move, rotate and scale -- which take the
//! selection and then clicks on the plane.

use super::modify_copy::{circular_pattern, linear_pattern, mirror, selected, transform};
use super::*;
use crate::app::Status;
use crate::view::View;

/// Change the sketch through `edit`, which says why not by failing, and keep it only if it solves.
fn solved(tool: &mut SketchTool, done: &str, edit: impl FnOnce(&mut Sketch) -> Result<(), &'static str>) -> Status {
    let before = tool.sketch.clone();
    match edit(&mut tool.sketch) {
        Ok(()) if tool.sketch.solve() => {
            tool.remember(before);
            tool.refresh();
            Status::Info(done.into())
        }
        Ok(()) => {
            tool.sketch = before;
            Status::Warning("The constraints leave no room for that".into())
        }
        Err(why) => {
            tool.sketch = before;
            Status::Warning(why.into())
        }
    }
}

/// Extend, split or offset the curve under the pointer.
pub(crate) fn curve_tool_at(tool: &mut SketchTool, view: &View, cursor: egui::Pos2) -> Status {
    let (Some(id), Some(at)) = (tool.curve_at(view, cursor), tool.under(view, cursor)) else {
        return Status::Info(tool.draw.hint().into());
    };
    if tool.draw != Draw::Offset && tool.sketch.curve(id).is_some_and(|c| c.reference) {
        return Status::Warning("Copied model edges cannot be changed".into());
    }
    let point = tool.point_at(view, cursor, &[]);
    let distance = tool.settings.offset_distance;
    match tool.draw {
        Draw::Extend => solved(tool, "Extended", |s| super::modify_ends::extend(s, id, at)),
        Draw::Split => solved(tool, "Split in two", |s| super::modify_ends::split(s, id, at, point)),
        _ => solved(tool, "Offset", |s| super::modify_offset::offset(s, id, at, distance).map(|_| ())),
    }
}

/// Cut the corner under the pointer off at the chamfer distance.
pub(crate) fn chamfer_at(tool: &mut SketchTool, view: &View, cursor: egui::Pos2) -> Status {
    let Some(corner) = tool.point_at(view, cursor, &[]) else {
        return Status::Info("Click a corner where two lines meet".into());
    };
    let distance = tool.settings.chamfer_distance;
    solved(tool, "Cut the corner off", |s| super::modify_ends::chamfer(s, corner, distance).map(|_| ()))
}

/// Add the curve under the pointer to the selection, or drop it.
fn toggle_curve(tool: &mut SketchTool, view: &View, cursor: egui::Pos2) -> Status {
    let Some(id) = tool.curve_at(view, cursor) else { return Status::Info("Shift-click curves to select them".into()) };
    let pick = Pick::Curve(id);
    match tool.selection.iter().position(|&p| p == pick) {
        Some(i) => {
            tool.selection.remove(i);
        }
        None => tool.selection.push(pick),
    }
    Status::Info(tool.draw.hint().into())
}

/// A click of a selection tool: with Shift, or while nothing is selected, it picks curves; after
/// that the mirror's click is on its line, the others' are on the plane until they have enough.
pub(crate) fn selection_tool_at(
    tool: &mut SketchTool,
    view: &View,
    cursor: egui::Pos2,
    (shift, spacing): (bool, Option<f64>),
) -> Status {
    let (curves, points) = selected(tool);
    if shift || (curves.is_empty() && points.is_empty() && tool.clicks.is_empty()) {
        tool.clicks.clear();
        return toggle_curve(tool, view, cursor);
    }
    if tool.draw == Draw::Mirror {
        let Some(line) = tool.curve_at(view, cursor) else {
            return Status::Info("Click the line to mirror across".into());
        };
        let before = tool.sketch.clone();
        return match mirror(&mut tool.sketch, &curves, line) {
            Ok(n) => {
                tool.sketch.solve();
                tool.remember(before);
                tool.refresh();
                Status::Info(format!("Mirrored {n} curve{}", if n == 1 { "" } else { "s" }))
            }
            Err(why) => {
                tool.sketch = before;
                Status::Warning(why.into())
            }
        };
    }
    let Some(click) = tool.click_at(view, cursor, spacing) else { return Status::Info(String::new()) };
    tool.clicks.push(click);
    if tool.clicks.len() < tool.draw.selection_clicks() {
        return Status::Info(tool.draw.hint().into());
    }
    let at: Vec<[f64; 2]> = std::mem::take(&mut tool.clicks).iter().map(|c| c.at).collect();
    let (draw, settings) = (tool.draw, &tool.settings);
    let mut sketch = tool.sketch.clone();
    if !apply(&mut sketch, draw, (&curves, &points), &at, settings) {
        return Status::Warning("Nothing to do with those clicks".into());
    }
    let label = draw.label();
    solved(tool, &format!("{label} done"), |s| {
        *s = sketch;
        Ok(())
    })
}

/// What a selection tool does with its clicks on the plane, into `sketch`, without solving.
pub(crate) fn apply(
    sketch: &mut Sketch,
    draw: Draw,
    (curves, points): (&[CurveId], &[PointId]),
    at: &[[f64; 2]],
    settings: &super::settings::Settings,
) -> bool {
    match draw {
        Draw::LinearPattern if !curves.is_empty() => linear_pattern(sketch, curves, at[0], at[1], settings),
        Draw::CircularPattern if !curves.is_empty() => circular_pattern(sketch, curves, at[0], settings),
        Draw::Move | Draw::Rotate | Draw::Scale => return transform(sketch, draw, (curves, points), at),
        _ => return false,
    }
    true
}
