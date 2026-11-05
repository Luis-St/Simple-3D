//! The lines a profile can turn about: one under the pointer, or an axis of the world.

use super::*;
use crate::app::App;
use crate::snap::{nearest_on_edge, CATCH_PIXELS};
use crate::view::View;
use simple3d_sketch::Shape;

/// How far out of the profile's plane, in millimetres, an edge or axis may still count as in it.
const IN_PLANE: f64 = 1e-4;

/// How far either way from the origin a world axis is caught.
const AXIS_REACH: f64 = 1000.0;

impl App {
    /// The line under `cursor` that `profile` can turn about: a line of its own sketch, else a
    /// straight edge of the model, else a shown axis of the world. `Err` with why for one there
    /// that is not in the profile's plane.
    pub(crate) fn revolve_axis_under(
        &self,
        view: &View,
        cursor: egui::Pos2,
        profile: &Target,
    ) -> Option<Result<AxisPick, &'static str>> {
        let project = |p: Vec3| view.project(p).map(|(screen, _)| screen);
        let frame = profile.frame();
        let middle = super::angle::flat_middle(profile);
        if let Target::Region(hit, _) = profile {
            let sketch = &hit.sketch;
            let near = sketch
                .curves
                .iter()
                .filter_map(|curve| match curve.shape {
                    Shape::Line { a, b } => {
                        let flat = (sketch.at(a), sketch.at(b));
                        let world = (lift(&frame, flat.0), lift(&frame, flat.1));
                        let (_, gap) = nearest_on_edge(world.0, world.1, project, cursor, CATCH_PIXELS)?;
                        let stored = RevolveAxis::Line(curve.id);
                        Some((gap, AxisPick { world, flat, stored, middle, source: AxisSource::Picked }))
                    }
                    _ => None,
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, pick)) = near {
                return Some(Ok(pick));
            }
        }
        if let Some(edge) = self.nearest_model_edge(view, cursor) {
            return Some(fixed(&frame, edge, middle).ok_or("That edge is not in the plane of the profile"));
        }
        let shown = self.shown_axes();
        let caught = (0..3).filter(|&k| shown[k]).find(|&k| {
            let mut direction = Vec3::ZERO;
            direction.set(k, 1.0);
            let (a, b) = (direction * -AXIS_REACH, direction * AXIS_REACH);
            nearest_on_edge(a, b, project, cursor, CATCH_PIXELS).is_some()
        });
        caught.map(|k| world(profile, k))
    }
}

/// The sketch point `p` on `frame`, in the world.
pub(crate) fn lift(frame: &simple3d_core::xform::Xform, p: [f64; 2]) -> Vec3 {
    frame.point(Vec3::new(p[0], p[1], 0.0))
}

/// A line through world points `ends` as an axis in `frame`'s plane, if it lies in it.
fn fixed(frame: &simple3d_core::xform::Xform, ends: (Vec3, Vec3), middle: [f64; 2]) -> Option<AxisPick> {
    let inverse = frame.inverse();
    let (a, b) = (inverse.point(ends.0), inverse.point(ends.1));
    if a.z.abs() > IN_PLANE || b.z.abs() > IN_PLANE || ((b.x - a.x).hypot(b.y - a.y)) < 1e-9 {
        return None;
    }
    let flat = ([a.x, a.y], [b.x, b.y]);
    Some(AxisPick { world: ends, flat, stored: RevolveAxis::Fixed(flat.0, flat.1), middle, source: AxisSource::Picked })
}

/// The world's axis `k` (X, Y, Z) as an axis for `profile`: the axis itself when it lies in the
/// profile's plane, else the line in the plane right over it when it runs parallel to the plane.
/// Its ends sit either side of the profile, so the line drawn for it stays near the profile.
pub(crate) fn world(profile: &Target, k: usize) -> Result<AxisPick, &'static str> {
    let frame = profile.frame();
    let inverse = frame.inverse();
    let mut direction = Vec3::ZERO;
    direction.set(k, 1.0);
    let origin = inverse.point(Vec3::ZERO);
    let along = inverse.point(direction) - origin;
    let flat_length = along.x.hypot(along.y);
    if flat_length < 1e-9 || along.z.abs() > 1e-6 * along.length() {
        return Err("That axis is not parallel to the plane of the profile");
    }
    let d = [along.x / flat_length, along.y / flat_length];
    let middle = super::angle::flat_middle(profile);
    // The point of the line nearest the profile's middle, and the profile's reach from there.
    let t = (middle[0] - origin.x) * d[0] + (middle[1] - origin.y) * d[1];
    let foot = [origin.x + d[0] * t, origin.y + d[1] * t];
    let reach =
        profile.outline().outer.iter().map(|p| (p[0] - middle[0]).hypot(p[1] - middle[1])).fold(0.0, f64::max) + 10.0;
    let flat = ([foot[0] - d[0] * reach, foot[1] - d[1] * reach], [foot[0] + d[0] * reach, foot[1] + d[1] * reach]);
    let world = (lift(&frame, flat.0), lift(&frame, flat.1));
    let stored = RevolveAxis::Fixed(flat.0, flat.1);
    Ok(AxisPick { world, flat, stored, middle, source: AxisSource::World(k) })
}
