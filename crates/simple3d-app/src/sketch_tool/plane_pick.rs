//! The editor's first click, which picks the plane to sketch on, and the frame a face's plane gets.

use super::*;
use crate::app::{App, Status};
use crate::view::View;
use simple3d_geom::plane_cut::CutPlane;

/// The first click: an origin plane or a face of the model puts the sketch on it, anywhere else on the
/// plane being worked on (the active workplane, or the ground). The origin planes are drawn over the
/// model, so they win over the face behind them.
pub(crate) fn pick_plane(app: &mut App, view: &View, cursor: egui::Pos2) {
    if let Some(plane) = super::origin::plane_at(view, cursor) {
        app.place_sketch(plane.frame(), None);
        return;
    }
    match app.face_under(view, cursor) {
        Some((Ok(hit), _)) => {
            let face = CutPlane { point: hit.captured.origin, normal: hit.captured.normal };
            let (u, v) = sketch_axes(face.normal);
            let frame = plane_frame(face.point, u, v);
            app.place_sketch(frame, Some(hit.owner));
        }
        Some((Err(why), _)) => app.status = Status::Warning(why.into()),
        None => {
            let frame = match app.workplane() {
                Some(plane) => plane.frame(),
                None => Xform::from_translation(Vec3::ZERO),
            };
            app.place_sketch(frame, None);
        }
    }
}

/// The axes of a sketch on a face facing `normal`: its X level, so that looking straight at it, as
/// the camera does without rolling, puts X to the right and Y up. Only a face looking straight up or
/// down has no level direction of its own; there X is the world's, turned to keep the frame
/// right-handed as seen from the face's side, as the origin planes are.
pub(crate) fn sketch_axes(normal: Vec3) -> (Vec3, Vec3) {
    let n = normal.normalized();
    let u = if n.z.abs() > 0.999 {
        Vec3::new(n.z.signum(), 0.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, 1.0).cross(n).normalized()
    };
    (u, n.cross(u))
}

/// A frame with origin `origin` and axes `u`, `v` and their normal.
pub(crate) fn plane_frame(origin: Vec3, u: Vec3, v: Vec3) -> Xform {
    let rotation = Xform::rotation_of_axes(u, v, u.cross(v));
    Xform::from_pos_rot(origin, rotation)
}
