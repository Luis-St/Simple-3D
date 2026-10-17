//! The snap positions of the face a shape is being put down on (issue 128): its centre, where the
//! shape sits in the middle, and its corners, where it sits flush into the corner. Both turn the
//! shape about the face's normal so its sides run along the face's edges.

use crate::app::App;
use crate::snap::{point_key, FeatureKind, SnapMark, CATCH_PIXELS};
use crate::view::View;
use simple3d_core::scene::NodeId;
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;
use std::collections::HashMap;

/// How far off the face's plane a point may be and still be on it, in millimetres.
const ON_PLANE: f64 = 1e-4;

/// A flat face under the pointer.
pub(super) struct Face {
    centre: Vec3,
    /// Each corner, with the unit directions of its two edges away from it.
    corners: Vec<(Vec3, [Vec3; 2])>,
    outline: Vec<(Vec3, Vec3)>,
}

/// Where on a face the shape was caught.
pub(super) enum Caught {
    Centre,
    /// A corner, with the directions of its edges.
    Corner([Vec3; 2]),
}

impl Face {
    /// The face's snap position nearest the pointer and in reach of it.
    pub(super) fn catch(&self, view: &View, cursor: egui::Pos2) -> Option<(Vec3, Caught)> {
        let points = std::iter::once((self.centre, Caught::Centre))
            .chain(self.corners.iter().map(|&(at, edges)| (at, Caught::Corner(edges))));
        points
            .filter_map(|(at, caught)| Some((view.project(at)?.0.distance(cursor), at, caught)))
            .filter(|(distance, _, _)| *distance <= CATCH_PIXELS)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, at, caught)| (at, caught))
    }

    /// The face's outline and corners, drawn as what was caught at `at`.
    pub(super) fn mark(&self, at: Vec3) -> SnapMark {
        let corners = self.corners.iter().map(|(corner, _)| *corner).collect();
        SnapMark { at, corners, edges: self.outline.clone() }
    }

    /// `turn` spun about the face's normal by the least angle that lines its X up with an edge:
    /// the longest one for the centre, the corner's own for a corner.
    pub(super) fn aligned(&self, turn: &Xform, caught: &Caught) -> Xform {
        let edges: Vec<Vec3> = match caught {
            Caught::Corner(edges) => edges.to_vec(),
            Caught::Centre => self
                .outline
                .iter()
                .max_by(|a, b| (a.1 - a.0).length().total_cmp(&(b.1 - b.0).length()))
                .map(|(a, b)| (*b - *a).normalized())
                .into_iter()
                .collect(),
        };
        let (x, y, normal) = (turn.axis(0), turn.axis(1), turn.axis(2));
        let quarter = std::f64::consts::FRAC_PI_2;
        // Mod a quarter turn, since the shape's sides line up with an edge either way.
        let spin = |edge: &Vec3| {
            let angle = edge.dot(y).atan2(edge.dot(x));
            angle - (angle / quarter).round() * quarter
        };
        let Some(angle) = edges.iter().map(spin).min_by(|a, b| a.abs().total_cmp(&b.abs())) else { return *turn };
        let spun = x * angle.cos() + y * angle.sin();
        let rotation = Xform::rotation_of_axes(spun, normal.cross(spun), normal);
        Xform::from_pos_rot(Vec3::ZERO, rotation)
    }
}

impl App {
    /// The flat face of a body that `at` lies on, its plane facing along `normal`, leaving `exclude`
    /// out. `None` off any flat face, on a curved surface for one.
    pub(super) fn face_at(&self, at: Vec3, normal: Vec3, exclude: &[NodeId]) -> Option<Face> {
        let on_plane = |p: Vec3| (p - at).dot(normal).abs() <= ON_PLANE;
        for (id, mesh) in self.snap_bodies(exclude) {
            let snaps = self.snaps_of(id, &mesh);
            for feature in &snaps.features {
                if feature.kind != FeatureKind::FaceCentre || !on_plane(feature.point) {
                    continue;
                }
                let Some(outline) = feature.face.and_then(|face| snaps.faces.get(face as usize)) else { continue };
                if outline.iter().all(|&(a, b)| on_plane(a) && on_plane(b)) && encloses(outline, at, normal) {
                    let corners = corners_of(outline);
                    return Some(Face { centre: feature.point, corners, outline: outline.clone() });
                }
            }
        }
        None
    }
}

/// Whether `at` is inside `outline`, in its plane, by counting the edges a ray from it crosses;
/// holes in a face are outline too, so a point in one is outside.
fn encloses(outline: &[(Vec3, Vec3)], at: Vec3, normal: Vec3) -> bool {
    let helper = if normal.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
    let u = normal.cross(helper).normalized();
    let v = normal.cross(u);
    let flat = |p: Vec3| ((p - at).dot(u), (p - at).dot(v));
    let mut inside = false;
    for &(a, b) in outline {
        let ((ax, ay), (bx, by)) = (flat(a), flat(b));
        if (ay > 0.0) != (by > 0.0) && ax + (bx - ax) * (0.0 - ay) / (by - ay) > 0.0 {
            inside = !inside;
        }
    }
    inside
}

/// The points of `outline` where it turns, each with its two edges' directions away from it. A point
/// two edges meet at in a straight line is no corner.
fn corners_of(outline: &[(Vec3, Vec3)]) -> Vec<(Vec3, [Vec3; 2])> {
    let mut ends: HashMap<(i64, i64, i64), (Vec3, Vec<Vec3>)> = HashMap::new();
    for &(a, b) in outline {
        for (from, to) in [(a, b), (b, a)] {
            ends.entry(point_key(from)).or_insert((from, Vec::new())).1.push((to - from).normalized());
        }
    }
    let mut corners: Vec<(Vec3, [Vec3; 2])> = ends
        .into_values()
        .filter_map(|(at, edges)| match edges[..] {
            [a, b] if a.cross(b).length() > 1e-6 => Some((at, [a, b])),
            _ => None,
        })
        .collect();
    // In a fixed order, so equally near corners break ties the same way every run.
    corners
        .sort_by(|a, b| (a.0.x, a.0.y, a.0.z).partial_cmp(&(b.0.x, b.0.y, b.0.z)).unwrap_or(std::cmp::Ordering::Equal));
    corners
}
