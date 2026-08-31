//! Snap features: the notable points on a body's surface that a drag can catch
//! onto and the measure tool can pick (issues 68, 69).
//!
//! A body offers three kinds of them -- its vertices, the midpoints of its edges
//! and the centres of its flat faces -- and both features that pick geometry
//! reach for the same three, so they live in one place. The input is a body's
//! own world-space mesh, the one `Evaluated::node_meshes` already holds, so a
//! feature is in world space from the moment it is found and nothing has to
//! transform it again.
//!
//! Meshes arrive triangulated, with a box face split into two triangles and each
//! triangle carrying its own unwelded copies of its corners. Left as they are,
//! "vertices" would be three per triangle and "face centres" would be triangle
//! centroids -- two of them per box face, neither at the face's middle. So the
//! mesh is welded first, which recovers the shared vertices, and coplanar
//! triangles are merged back into the flat face they came from, which recovers
//! the one centre a user means by it.

use simple3d_geom::{Mesh, Vec3};
use std::collections::HashMap;

/// Which of the three notable points a feature is. Kept so the interface can
/// name what a snap or a measurement caught, and so a preference could one day
/// turn a kind off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureKind {
    Vertex,
    EdgeMidpoint,
    FaceCentre,
}

impl FeatureKind {
    pub fn label(self) -> &'static str {
        match self {
            FeatureKind::Vertex => "vertex",
            FeatureKind::EdgeMidpoint => "edge midpoint",
            FeatureKind::FaceCentre => "face centre",
        }
    }
}

/// How near the pointer a feature has to project to be caught, in screen pixels.
/// One radius for the measure tool and for geometry snapping, so a feature feels
/// the same to reach for whichever is doing the reaching.
pub const CATCH_PIXELS: f32 = 12.0;

/// One catchable point on a body, in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feature {
    pub point: Vec3,
    pub kind: FeatureKind,
}

/// Every snap feature of one body's mesh: its vertices, its edge midpoints and
/// its face centres, in that order so a nearest-point search that breaks ties by
/// index prefers the more exact kind.
///
/// The mesh is welded first so a vertex is one point rather than the several
/// copies a triangulated surface stores. What counts as a real edge or corner is
/// then decided by the creases in the surface, not by how it was triangulated: an
/// edge shared by two triangles that lie in the same plane -- a face's own
/// diagonal, or a spoke of the fan a flat cap is triangulated as -- is interior
/// to a face and no edge at all, and a vertex that only such edges touch (a cap's
/// fan centre) is no corner. So only edges at a crease or a boundary become edge
/// features, only the vertices those touch become vertex features, and the flat
/// faces between the creases each report one centre.
pub fn features_of(mesh: &Mesh) -> Vec<Feature> {
    let welded = mesh.weld();
    let n = welded.indices.len();
    if n == 0 {
        return Vec::new();
    }
    let normals: Vec<Vec3> = welded.indices.iter().map(|&t| welded.triangle_normal(t)).collect();

    // Which triangles border each undirected edge, so coplanar neighbours can be
    // found without an O(n^2) sweep.
    let mut edge_tris: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, tri) in welded.indices.iter().enumerate() {
        for e in 0..3 {
            let (a, b) = (tri[e], tri[(e + 1) % 3]);
            edge_tris.entry((a.min(b), a.max(b))).or_default().push(i);
        }
    }

    // An edge is interior to a flat face when exactly two triangles share it and
    // they face the same way; anything else -- a crease between differently
    // facing triangles, or a boundary bordered by one -- is a real edge.
    let interior = |tris: &[usize]| tris.len() == 2 && normals[tris[0]].dot(normals[tris[1]]) > 0.9999;

    let mut features = Vec::new();
    let mut corner_indices: Vec<u32> = Vec::new();
    for (&(a, b), tris) in &edge_tris {
        if interior(tris) {
            continue;
        }
        let mid = (welded.positions[a as usize] + welded.positions[b as usize]) * 0.5;
        features.push(Feature { point: mid, kind: FeatureKind::EdgeMidpoint });
        corner_indices.push(a);
        corner_indices.push(b);
    }
    corner_indices.sort_unstable();
    corner_indices.dedup();
    // Vertices before the edge midpoints already pushed: the whole list is
    // re-ordered vertices-first below, so the exact kind wins a screen-space tie.
    let mut vertices: Vec<Feature> =
        corner_indices.iter().map(|&i| Feature { point: welded.positions[i as usize], kind: FeatureKind::Vertex }).collect();

    // Coplanar triangles joined into one face, so a box's two triangles per side
    // report one centre in the middle rather than two triangle centroids.
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for tris in edge_tris.values() {
        if interior(tris) {
            let (i, j) = (tris[0], tris[1]);
            let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
            if ri != rj {
                parent[ri] = rj;
            }
        }
    }
    // Each face's centroid, weighted by triangle area so a face split into uneven
    // triangles still reports its true middle.
    let mut sums: HashMap<usize, (Vec3, f64)> = HashMap::new();
    for (i, &tri) in welded.indices.iter().enumerate() {
        let (a, b, c) =
            (welded.positions[tri[0] as usize], welded.positions[tri[1] as usize], welded.positions[tri[2] as usize]);
        let area = (b - a).cross(c - a).length() * 0.5;
        let centroid = (a + b + c) * (1.0 / 3.0);
        let root = find(&mut parent, i);
        let entry = sums.entry(root).or_insert((Vec3::ZERO, 0.0));
        entry.0 = entry.0 + centroid * area;
        entry.1 += area;
    }
    let mut centres: Vec<Feature> = sums
        .values()
        .filter(|(_, area)| *area > 1e-9)
        .map(|(sum, area)| Feature { point: *sum * (1.0 / area), kind: FeatureKind::FaceCentre })
        .collect();

    // Deterministic order within each kind, so two runs offer features in the
    // same sequence and a nearest-point tie breaks the same way.
    let by_point = |a: &Feature, b: &Feature| {
        (a.point.x, a.point.y, a.point.z).partial_cmp(&(b.point.x, b.point.y, b.point.z)).unwrap_or(std::cmp::Ordering::Equal)
    };
    vertices.sort_by(by_point);
    features.sort_by(by_point);
    centres.sort_by(by_point);

    let mut out = vertices;
    out.append(&mut features);
    out.append(&mut centres);
    out
}

/// The feature nearest the cursor on screen, within `max_pixels`, together with
/// how far away it landed.
///
/// The match is by screen distance, not world distance: what a user means by
/// "that corner" is the one under the pointer, and two corners far apart in the
/// model can sit close together in the frame. `project` returns `None` for a
/// point that does not land on screen, which is skipped.
pub fn nearest_on_screen<'a>(
    features: &'a [Feature],
    project: impl Fn(Vec3) -> Option<egui::Pos2>,
    cursor: egui::Pos2,
    max_pixels: f32,
) -> Option<(&'a Feature, f32)> {
    let mut best: Option<(&Feature, f32)> = None;
    for feature in features {
        let Some(screen) = project(feature.point) else { continue };
        let distance = (screen - cursor).length();
        if distance > max_pixels {
            continue;
        }
        if best.is_none_or(|(_, d)| distance < d) {
            best = Some((feature, distance));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use simple3d_geom::primitives;

    fn has_point(features: &[Feature], kind: FeatureKind, p: Vec3) -> bool {
        features.iter().any(|f| f.kind == kind && (f.point - p).length() < 1e-6)
    }

    #[test]
    fn a_box_offers_its_eight_corners_twelve_edge_midpoints_and_six_face_centres() {
        // A cube from -5 to 5 on every axis: the counts are exact once the
        // triangulated, unwelded mesh has been welded and its faces merged.
        let mesh = primitives::box_mesh(10.0, 10.0, 10.0);
        let features = features_of(&mesh);

        let count = |kind: FeatureKind| features.iter().filter(|f| f.kind == kind).count();
        assert_eq!(count(FeatureKind::Vertex), 8, "a box has eight corners, not three per triangle");
        assert_eq!(count(FeatureKind::EdgeMidpoint), 12, "a box has twelve edges");
        assert_eq!(count(FeatureKind::FaceCentre), 6, "a box has six faces, not two triangles per side");

        // The named corner, edge midpoint and face centre are all really there.
        assert!(has_point(&features, FeatureKind::Vertex, Vec3::new(5.0, 5.0, 5.0)));
        assert!(has_point(&features, FeatureKind::EdgeMidpoint, Vec3::new(0.0, 5.0, 5.0)));
        assert!(has_point(&features, FeatureKind::FaceCentre, Vec3::new(0.0, 0.0, 5.0)), "the top face centre is missing");
    }

    #[test]
    fn a_face_centre_sits_in_the_middle_of_the_face_not_at_a_triangles_centroid() {
        // The whole reason to merge coplanar triangles: a triangle centroid of
        // the top face would be at (+/-, +/-, 5) off toward a corner, never at
        // its middle.
        let mesh = primitives::box_mesh(20.0, 8.0, 4.0);
        let features = features_of(&mesh);
        assert!(
            has_point(&features, FeatureKind::FaceCentre, Vec3::new(0.0, 0.0, 2.0)),
            "the top face centre was not at the middle of the face"
        );
        assert!(has_point(&features, FeatureKind::FaceCentre, Vec3::new(10.0, 0.0, 0.0)), "the +X face centre is missing");
    }

    #[test]
    fn the_nearest_feature_on_screen_is_the_one_under_the_cursor_within_reach() {
        let features = vec![
            Feature { point: Vec3::new(0.0, 0.0, 0.0), kind: FeatureKind::Vertex },
            Feature { point: Vec3::new(100.0, 0.0, 0.0), kind: FeatureKind::Vertex },
        ];
        // A trivial orthographic-ish projection: X and Y straight to screen.
        let project = |p: Vec3| Some(egui::pos2(p.x as f32, p.y as f32));

        let cursor = egui::pos2(3.0, 2.0);
        let (found, distance) = nearest_on_screen(&features, project, cursor, 10.0).unwrap();
        assert_eq!(found.point, Vec3::ZERO);
        assert!((distance - (3.0f32 * 3.0 + 2.0 * 2.0).sqrt()).abs() < 1e-4);

        // Nothing within reach returns nothing, which is how a drag knows to fall
        // back to the grid.
        assert!(nearest_on_screen(&features, project, egui::pos2(50.0, 50.0), 10.0).is_none());
    }
}
