//! Starting a stroke and laying one dab.

use super::*;
use crate::app::Status;
use simple3d_geom::brush::{dab, Mode, Surface};

/// The longest edge a dab leaves under it with adding detail off, as a share of the radius: a coarse
/// mesh (a converted box has one vertex per corner) still gets a few vertices across the circle to
/// move, rather than none at all. Finer meshes are left as they are.
const COARSEST: f64 = 0.5;

impl App {
    /// Start a stroke on the brush's target. `false` when there is none or it is not evaluated yet.
    pub(crate) fn begin_brush_stroke(&mut self) -> bool {
        let Some(id) = target(self) else { return false };
        if !self.begin_stroke(id) {
            return false;
        }
        let keeping = self.settings.last_brush.keeping();
        let sculpt = self.sculpt.as_mut().expect("the stroke began");
        sculpt.weigh(&keeping);
        if sculpt.lagging {
            self.status = Status::Info(
                "Brushing a mesh inside a boolean or pattern: the result there shows once the stroke ends".into(),
            );
        }
        true
    }

    /// One dab in `mode` where the ray from `origin` along `dir` meets the working copy. Where it
    /// landed, or `None` when it missed.
    pub(crate) fn brush_dab(&mut self, origin: Vec3, dir: Vec3, mode: Mode) -> Option<Vec3> {
        let brush = self.settings.last_brush;
        let keeping = brush.keeping();
        let sculpt = self.sculpt.as_mut().filter(|sculpt| sculpt.stroke.is_some())?;
        let (t, mut triangle) = sculpt.ray(origin, dir)?;
        let centre = origin + dir * t;
        let edge = match brush.detail {
            true => brush.edge,
            false => brush.radius * COARSEST,
        };
        if sculpt.add_detail(triangle, centre, brush.radius, edge) {
            sculpt.weigh(&keeping);
            triangle = sculpt.ray(origin, dir)?.1;
        }
        let region = sculpt.within(triangle, centre, brush.radius);
        let stroke = sculpt.stroke.as_ref().expect("checked above");
        let surface = Surface {
            positions: &sculpt.renderable.mesh.positions,
            normals: &sculpt.normals,
            neighbours: &sculpt.topology.neighbours,
            weights: &stroke.weights,
            rest: &|v| stroke.rest(v),
        };
        let moves = dab(&brush, mode, &surface, &region);
        sculpt.move_vertices(&moves);
        Some(centre)
    }

    /// End the stroke, keeping it as one undo step.
    pub(crate) fn end_brush_stroke(&mut self) {
        let mode = self.settings.last_brush.mode.label().to_lowercase();
        self.end_stroke(&format!("Brush ({mode})"));
    }
}
