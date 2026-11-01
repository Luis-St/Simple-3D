//! The clicks of the tools that pick something rather than draw: regions, the revolve axis, face
//! edges to copy, and the trim and fillet tools.

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
    let helper = tool.settings.copy_as_helper;
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
    let radius = tool.settings.fillet_radius;
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

impl SketchTool {
    /// What the tool in use would act on under the pointer, lit while it hovers there.
    pub(crate) fn hover_at(&self, view: &View, cursor: egui::Pos2, shift: bool) -> Option<Pick> {
        let curve = || self.curve_at(view, cursor).map(Pick::Curve);
        let point = || self.point_at(view, cursor, &[]).map(Pick::Point);
        match self.draw {
            Draw::Select => self.pick_at(view, cursor),
            Draw::Axis | Draw::Trim | Draw::Extend | Draw::Split | Draw::Offset | Draw::Mirror => curve(),
            draw if draw.uses_selection() && (shift || self.clicks.is_empty()) => curve().or_else(point),
            Draw::Dimension | Draw::Constrain(_) => {
                self.pick_at(view, cursor).filter(|p| !matches!(p, Pick::Constraint(_)))
            }
            Draw::Regions | Draw::CopyEdges => None,
            _ => point(),
        }
    }

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
