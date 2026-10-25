//! The three origin planes a sketch can be put on, as in other CAD programs: a small orange square
//! for each of XY, XZ and YZ at the world origin, shown while the editor waits for its plane.
//!
//! Each square sits in the positive quarter of its plane, so together they make the corner of a cube
//! in the +X +Y +Z direction whatever the view; where two cover each other the nearer one is hit. They keep one size on screen whatever the zoom, and a click on one
//! wins over the face of the model behind it.

use super::pointer::plane_frame;
use super::*;
use crate::theme::token;
use crate::view::View;

/// A square's side on screen, in pixels.
const SIDE: f64 = 64.0;
/// The gap the squares keep from the origin and from each other, in pixels.
const GAP: f64 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OriginPlane {
    XY,
    XZ,
    YZ,
}

impl OriginPlane {
    pub(crate) const ALL: [OriginPlane; 3] = [OriginPlane::XY, OriginPlane::XZ, OriginPlane::YZ];

    /// The plane's two axes, `u x v` its normal: XY faces up, XZ toward the front view's eye (-Y)
    /// and YZ toward the right view's (+X), so the camera turns to the view the plane is named for.
    fn axes(self) -> (Vec3, Vec3) {
        let (x, y, z) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0));
        match self {
            OriginPlane::XY => (x, y),
            OriginPlane::XZ => (x, z),
            OriginPlane::YZ => (y, z),
        }
    }

    /// The sketch frame on this plane, through the world origin.
    pub(crate) fn frame(self) -> Xform {
        let (u, v) = self.axes();
        plane_frame(Vec3::ZERO, u, v)
    }

    /// The square's corners in the world for `view`, always in the plane's positive quarter so the
    /// three stay put as the view turns.
    fn corners(self, view: &View) -> [Vec3; 4] {
        let (u, v) = self.axes();
        let mm = 1.0 / view.pixels_per_mm().max(1e-9);
        let (near, far) = (GAP * mm, (GAP + SIDE) * mm);
        [(near, near), (far, near), (far, far), (near, far)].map(|(s, t)| u * s + v * t)
    }

    /// The middle of the square in the world for `view`, where tests click.
    #[cfg(test)]
    pub(crate) fn middle(self, view: &View) -> Vec3 {
        let [a, _, c, _] = self.corners(view);
        (a + c) * 0.5
    }

    /// The square on screen, if all of it is in front of the eye.
    fn screen(self, view: &View) -> Option<Vec<egui::Pos2>> {
        self.corners(view).iter().map(|&p| view.project(p).map(|(at, _)| at)).collect()
    }
}

/// Whether `p` lies inside the convex screen polygon `points`, either winding.
fn inside(points: &[egui::Pos2], p: egui::Pos2) -> bool {
    let n = points.len();
    let sides: Vec<f32> = (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            (b - a).x * (p - a).y - (b - a).y * (p - a).x
        })
        .collect();
    sides.iter().all(|s| *s >= 0.0) || sides.iter().all(|s| *s <= 0.0)
}

/// The origin plane under the pointer, the nearest to the eye where squares overlap. A square seen
/// edge-on has no area on screen and cannot be hit.
pub(crate) fn plane_at(view: &View, cursor: egui::Pos2) -> Option<OriginPlane> {
    OriginPlane::ALL
        .into_iter()
        .filter(|plane| plane.screen(view).is_some_and(|square| inside(&square, cursor)))
        .filter_map(|plane| {
            let frame = plane.frame();
            let hit = view.ray_plane(cursor, frame.t, frame.axis(2))?;
            Some((plane, (hit - view.eye()).length()))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(plane, _)| plane)
}

/// The three squares in orange, the hovered one stronger.
pub(crate) fn draw(painter: &egui::Painter, view: &View, hover: Option<OriginPlane>) {
    for plane in OriginPlane::ALL {
        let Some(square) = plane.screen(view) else { continue };
        let lit = hover == Some(plane);
        let fill = token::ACCENT.gamma_multiply(if lit { 0.45 } else { 0.18 });
        let stroke = egui::Stroke::new(
            if lit { 2.0_f32 } else { 1.2 },
            token::ACCENT.gamma_multiply(if lit { 1.0 } else { 0.8 }),
        );
        painter.add(egui::Shape::convex_polygon(square, fill, stroke));
    }
}
