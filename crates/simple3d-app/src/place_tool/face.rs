//! The snap positions of the face a shape is being put down on (issue 128): its centre, its corners
//! and its edges, which the shape's middle catches while the snap key is held. On the face the shape
//! is turned about its normal so its sides run along the face's edges.

use super::outline::{corners_of, encloses};
use crate::app::App;
use crate::snap::{nearest_on_edge, point_key, FeatureKind, SnapMark};
use crate::view::View;
use simple3d_core::scene::NodeId;
use simple3d_core::xform::Xform;
use simple3d_geom::Vec3;

/// How far off the face's plane a point may be and still be on it, in millimetres.
const ON_PLANE: f64 = 1e-4;

/// A flat face under the pointer.
#[derive(Clone, Debug)]
pub(crate) struct Face {
    centre: Vec3,
    normal: Vec3,
    /// Each corner, with the unit directions of its two edges away from it.
    corners: Vec<(Vec3, [Vec3; 2])>,
    outline: Vec<(Vec3, Vec3)>,
}

/// Where on a face the shape was caught.
#[derive(Clone, Copy, Debug)]
pub(super) enum Caught {
    Centre,
    /// A corner, with the directions of its edges.
    Corner([Vec3; 2]),
    /// An edge, with its direction.
    Edge(Vec3),
}

impl Face {
    /// Whether this is the same face as `other`.
    pub(super) fn same(&self, other: &Face) -> bool {
        point_key(self.centre) == point_key(other.centre) && self.normal.dot(other.normal) > 1.0 - 1e-9
    }

    /// The face's snap position nearest the pointer within `reach` pixels: the centre, corners and
    /// edges' middles, then anywhere along an edge. Points win over an edge in reach, so aiming at a
    /// corner never lands beside it (issue 78).
    pub(super) fn catch(&self, view: &View, cursor: egui::Pos2, reach: f32) -> Option<(Vec3, Caught)> {
        let middles = self.outline.iter().enumerate().map(|(i, &(a, b))| ((a + b) * 0.5, Some(i)));
        let points = std::iter::once((self.centre, None))
            .chain(self.corners.iter().map(|&(at, _)| (at, None)))
            .chain(middles)
            .enumerate();
        let nearest = |found: &mut dyn Iterator<Item = (f32, Vec3, usize, Option<usize>)>| {
            found.filter(|found| found.0 <= reach).min_by(|a, b| a.0.total_cmp(&b.0))
        };
        let point = nearest(
            &mut points.filter_map(|(index, (at, edge))| Some((view.project(at)?.0.distance(cursor), at, index, edge))),
        );
        let project = |p: Vec3| view.project(p).map(|(screen, _)| screen);
        let along = || {
            let mut on = self.outline.iter().enumerate().filter_map(|(i, &(a, b))| {
                let (at, distance) = nearest_on_edge(a, b, project, cursor, reach)?;
                Some((distance, at, usize::MAX, Some(i)))
            });
            nearest(&mut on)
        };
        let (_, at, index, edge) = point.or_else(along)?;
        let caught = match (index, edge) {
            (_, Some(i)) => Caught::Edge((self.outline[i].1 - self.outline[i].0).normalized()),
            (0, None) => Caught::Centre,
            (index, None) => Caught::Corner(self.corners[index - 1].1),
        };
        Some((at, caught))
    }

    /// The face's outline and corners, drawn as what was caught at `at`.
    pub(super) fn mark(&self, at: Vec3) -> SnapMark {
        let corners = self.corners.iter().map(|(corner, _)| *corner).collect();
        SnapMark { at, corners, edges: self.outline.clone() }
    }

    /// The turn that rests a shape on the face, spun about its normal by the least angle that lines
    /// its X up with an edge: the longest one for the centre or nothing caught, the caught corner's
    /// or edge's own otherwise.
    pub(super) fn aligned(&self, caught: Option<&Caught>) -> Xform {
        let turn = Xform::turn_z_to(self.normal);
        let edges: Vec<Vec3> = match caught {
            Some(Caught::Corner(edges)) => edges.to_vec(),
            Some(Caught::Edge(along)) => vec![*along],
            Some(Caught::Centre) | None => self
                .outline
                .iter()
                .max_by(|a, b| (a.1 - a.0).length().total_cmp(&(b.1 - b.0).length()))
                .map(|(a, b)| (*b - *a).normalized())
                .into_iter()
                .collect(),
        };
        let (x, y) = (turn.axis(0), turn.axis(1));
        let quarter = std::f64::consts::FRAC_PI_2;
        // Mod a quarter turn, since the shape's sides line up with an edge either way.
        let spin = |edge: &Vec3| {
            let angle = edge.dot(y).atan2(edge.dot(x));
            angle - (angle / quarter).round() * quarter
        };
        let Some(angle) = edges.iter().map(spin).min_by(|a, b| a.abs().total_cmp(&b.abs())) else { return turn };
        let spun = x * angle.cos() + y * angle.sin();
        let rotation = Xform::rotation_of_axes(spun, self.normal.cross(spun), self.normal);
        Xform::from_pos_rot(Vec3::ZERO, rotation)
    }
}

impl App {
    /// The flat face of the body hit at `at` on `triangle` of the evaluated mesh, its plane facing
    /// along `normal`, leaving `exclude` out. `None` off any flat face, on a curved surface for one.
    /// Only the body hit is searched, since a scene's every face each frame made the placing lag.
    pub(super) fn face_at(&self, at: Vec3, triangle: usize, normal: Vec3, exclude: &[NodeId]) -> Option<Face> {
        let owner = NodeId::from(self.evaluated.mesh.source(triangle));
        let (id, mesh) = self
            .snap_bodies(exclude)
            .into_iter()
            .find(|&(id, _)| id == owner || self.scene.is_ancestor_of(id, owner))?;
        let on_plane = |p: Vec3| (p - at).dot(normal).abs() <= ON_PLANE;
        let snaps = self.snaps_of(id, &mesh);
        for feature in &snaps.features {
            if feature.kind != FeatureKind::FaceCentre || !on_plane(feature.point) {
                continue;
            }
            let Some(outline) = feature.face.and_then(|face| snaps.faces.get(face as usize)) else { continue };
            if outline.iter().all(|&(a, b)| on_plane(a) && on_plane(b)) && encloses(outline, at, normal) {
                let corners = corners_of(outline);
                return Some(Face { centre: feature.point, normal, corners, outline: outline.clone() });
            }
        }
        None
    }
}
