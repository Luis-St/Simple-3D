//! The clicks of the tools that pick something rather than draw: regions, the revolve axis, face
//! edges to copy, and the trim, fillet and dimension tools.

use super::constrain::Rule;
use super::*;
use crate::app::{App, Status};
use crate::view::View;
use simple3d_sketch::{ConstraintKind, Shape};

/// Pick or drop the region under the pointer.
pub(crate) fn toggle_region(tool: &mut SketchTool, view: &View, cursor: egui::Pos2) -> Status {
    let Some(at) = tool.under(view, cursor) else { return Status::Info(String::new()) };
    let regions = simple3d_sketch::regions::find(&tool.sketch, 64);
    let Some(index) = regions.at(at) else {
        return Status::Info("No closed region there; close the outline first".into());
    };
    let held = tool.sketch.regions.iter().position(|pick| regions.resolve(pick) == Some(index));
    let pick = regions.pick(index, at);
    tool.change(|sketch| match held {
        Some(k) => {
            sketch.regions.remove(k);
        }
        None => sketch.regions.push(pick),
    });
    Status::Info(if held.is_some() { "Dropped the region" } else { "Picked the region" }.into())
}

/// Make the line under the pointer the revolve axis.
pub(crate) fn pick_axis(tool: &mut SketchTool, view: &View, cursor: egui::Pos2) -> Status {
    match tool.curve_at(view, cursor) {
        Some(id) if tool.sketch.curve(id).is_some_and(|c| matches!(c.shape, Shape::Line { .. })) => {
            tool.change(|sketch| sketch.axis = Some(id));
            tool.params.insert("feature".into(), simple3d_core::primitive::ParamValue::Choice(1));
            Status::Info("The solid now revolves about that line".into())
        }
        _ => Status::Info("Click a line to revolve about".into()),
    }
}

/// Copy the edges of the face under the pointer onto the sketch, dropped onto its plane.
pub(crate) fn copy_edges(app: &mut App, view: &View, cursor: egui::Pos2) {
    let hit = match app.face_under(view, cursor) {
        Some((Ok(hit), _)) => hit,
        Some((Err(why), _)) => {
            app.status = Status::Warning(why.into());
            return;
        }
        None => {
            app.status = Status::Info("Click a face of the model to copy its edges".into());
            return;
        }
    };
    let Some(tool) = app.sketch_tool.as_mut() else { return };
    let face = hit.captured.clone();
    let helper = tool.copy_as_helper;
    let to_sketch = |p: [f64; 2]| tool.local(face.origin + face.u * p[0] + face.v * p[1]);
    let loops: Vec<Vec<[f64; 2]>> = std::iter::once(&face.outline.outer)
        .chain(face.outline.holes.iter())
        .map(|points| points.iter().map(|&p| to_sketch(p)).collect())
        .collect();
    let added = tool.change(|sketch| {
        let mut added = 0;
        for points in &loops {
            for i in 0..points.len() {
                added +=
                    sketch.add_reference_line(points[i], points[(i + 1) % points.len()], helper).is_some() as usize;
            }
        }
        added
    });
    app.status = Status::Info(format!("Copied {added} edges of the face"));
}

/// Trim the curve under the pointer back to the curves crossing it.
pub(crate) fn trim_at(tool: &mut SketchTool, view: &View, cursor: egui::Pos2) -> Status {
    let (Some(id), Some(at)) = (tool.curve_at(view, cursor), tool.under(view, cursor)) else {
        return Status::Info("Click the piece of a curve to cut away".into());
    };
    if tool.sketch.curve(id).is_some_and(|c| c.reference) {
        return Status::Warning("Copied model edges cannot be trimmed".into());
    }
    match tool.change(|sketch| super::modify::trim(sketch, id, at)) {
        Ok(()) => Status::Info("Trimmed".into()),
        Err(why) => Status::Warning(why.into()),
    }
}

/// Round the corner under the pointer with the fillet radius.
pub(crate) fn fillet_at(tool: &mut SketchTool, view: &View, cursor: egui::Pos2) -> Status {
    let Some(corner) = tool.point_at(view, cursor, &[]) else {
        return Status::Info("Click a corner where two lines meet".into());
    };
    let radius = tool.fillet_radius;
    let before = tool.sketch.clone();
    match super::modify::fillet(&mut tool.sketch, corner, radius) {
        Ok(_) if tool.sketch.solve() => {
            tool.remember(before);
            tool.refresh();
            Status::Info("Rounded the corner".into())
        }
        Ok(_) => {
            tool.sketch = before;
            Status::Warning("The constraints leave no room to round that corner".into())
        }
        Err(why) => {
            tool.sketch = before;
            Status::Warning(why.into())
        }
    }
}

/// The dimension tool's click: a line or round curve dimensioned at once, a point waiting for a
/// second point or a line. The new dimension's popup opens, so its value can be typed.
pub(crate) fn dimension_at(tool: &mut SketchTool, view: &View, cursor: egui::Pos2) -> Status {
    let first = tool.clicks.first().and_then(|c| c.point);
    let picked = match (tool.pick_at(view, cursor), first) {
        (Some(Pick::Point(b)), Some(a)) if a != b => vec![Pick::Point(a), Pick::Point(b)],
        (Some(Pick::Point(a)), _) => {
            tool.clicks = vec![Click { at: tool.sketch.at(a), point: Some(a) }];
            return Status::Info("Now click a second point, or a line".into());
        }
        (Some(Pick::Curve(line)), Some(a)) => vec![Pick::Point(a), Pick::Curve(line)],
        (Some(Pick::Curve(curve)), None) => {
            if let Some(existing) = tool.dimension_of(curve) {
                tool.selection = vec![Pick::Constraint(existing)];
                tool.open_inspect(Pick::Curve(curve), cursor);
                return Status::Info("Already dimensioned; type its value".into());
            }
            vec![Pick::Curve(curve)]
        }
        _ => {
            tool.clicks.clear();
            return Status::Info("Click a line, a circle or a point".into());
        }
    };
    tool.clicks.clear();
    tool.selection = picked;
    match tool.apply(Rule::Dimension) {
        Ok(_) => {
            if let Some(&pick) = tool.selection.first() {
                tool.open_inspect(pick, cursor);
            }
            Status::Info("Dimensioned; type its value in the popup".into())
        }
        Err(why) => Status::Warning(why),
    }
}

impl SketchTool {
    /// The dimension sizing curve `id`: a line's length or a round curve's radius.
    pub(crate) fn dimension_of(&self, id: CurveId) -> Option<ConstraintId> {
        let ends = self.sketch.curve(id).and_then(|c| match c.shape {
            Shape::Line { a, b } => Some((a, b)),
            _ => None,
        });
        self.sketch
            .constraints
            .iter()
            .find(|c| match c.kind {
                ConstraintKind::Distance { a, b, .. } => {
                    ends.is_some_and(|(p, q)| (a, b) == (p, q) || (a, b) == (q, p))
                }
                ConstraintKind::Radius { curve, .. } => curve == id,
                _ => false,
            })
            .map(|c| c.id)
    }
}
