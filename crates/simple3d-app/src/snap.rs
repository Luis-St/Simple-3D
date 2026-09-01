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
    /// Anywhere along an edge, rather than one of its notable points. Not a
    /// feature the list carries -- it is what catching an edge *between* its
    /// ends reports (issue 78), so a measurement can start part-way along one.
    Edge,
    /// Anywhere along a world axis. The axes are lines, and a point on one is
    /// as real a place to measure from as a point on an edge, so they are caught
    /// the same way rather than only at the handful of places where they meet
    /// something.
    Axis,
    /// Anywhere along the line a principal plane leaves on a body's surface.
    ///
    /// The renderer draws, on the solid itself, where each plane through the
    /// origin cuts it -- the mark that says how much of the shape is below the
    /// build plate, or which side of zero a face is on. It is a line on the
    /// object, in the colour of the axis its plane is named by, and every other
    /// line in the picture can be caught, so this one is too.
    PlaneMark,
    /// Where a world axis passes through a body's surface. The axes run through
    /// the model whether or not any geometry corner is there, and a corner of the
    /// model on an axis is exactly the place a measurement usually wants, so the
    /// crossings are offered as corners and the run between two of them as an
    /// edge (issue 78).
    AxisCrossing,
}

impl FeatureKind {
    pub fn label(self) -> &'static str {
        match self {
            FeatureKind::Vertex => "vertex",
            FeatureKind::EdgeMidpoint => "edge midpoint",
            FeatureKind::FaceCentre => "face centre",
            FeatureKind::Edge => "edge",
            FeatureKind::Axis => "axis",
            FeatureKind::PlaneMark => "plane mark",
            FeatureKind::AxisCrossing => "axis crossing",
        }
    }
}

/// How near the pointer a feature has to project to be caught, in screen pixels.
/// One radius for the measure tool and for geometry snapping, so a feature feels
/// the same to reach for whichever is doing the reaching.
pub const CATCH_PIXELS: f32 = 12.0;

/// One catchable point on a body, in world space.
///
/// An edge feature also carries the edge it is the middle of, so a pointer that
/// is near the edge but nowhere near its midpoint can still catch the edge --
/// at the place along it that is actually being pointed at (issue 78).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feature {
    pub point: Vec3,
    pub kind: FeatureKind,
    pub span: Option<(Vec3, Vec3)>,
}

impl Feature {
    /// A feature that is a point and nothing more.
    pub fn point(point: Vec3, kind: FeatureKind) -> Feature {
        Feature { point, kind, span: None }
    }

    /// An edge, reported at its midpoint and carrying its two ends.
    pub fn edge(a: Vec3, b: Vec3) -> Feature {
        Feature { point: (a + b) * 0.5, kind: FeatureKind::EdgeMidpoint, span: Some((a, b)) }
    }
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
        features.push(Feature::edge(welded.positions[a as usize], welded.positions[b as usize]));
        corner_indices.push(a);
        corner_indices.push(b);
    }
    corner_indices.sort_unstable();
    corner_indices.dedup();
    // Vertices before the edge midpoints already pushed: the whole list is
    // re-ordered vertices-first below, so the exact kind wins a screen-space tie.
    let mut vertices: Vec<Feature> =
        corner_indices.iter().map(|&i| Feature::point(welded.positions[i as usize], FeatureKind::Vertex)).collect();

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
        .map(|(sum, area)| Feature::point(*sum * (1.0 / area), FeatureKind::FaceCentre))
        .collect();

    // Deterministic order within each kind, so two runs offer features in the
    // same sequence and a nearest-point tie breaks the same way.
    let by_point = |a: &Feature, b: &Feature| {
        (a.point.x, a.point.y, a.point.z)
            .partial_cmp(&(b.point.x, b.point.y, b.point.z))
            .unwrap_or(std::cmp::Ordering::Equal)
    };
    vertices.sort_by(by_point);
    features.sort_by(by_point);
    centres.sort_by(by_point);

    let mut out = vertices;
    out.append(&mut features);
    out.append(&mut centres);
    out
}

/// Where the world axes pass through one body, as features (issue 78).
///
/// The origin axes run through the model, and the place a measurement usually
/// wants -- where the axis leaves the body, where its centreline meets a face --
/// is often not a corner of the mesh at all, so nothing was there to catch. Each
/// crossing of the surface becomes a corner.
///
/// Only the crossings, and not the run between them: that stretch is inside the
/// material, where the renderer cuts the line out altogether, and a snap target
/// on a line nobody can see is a jump with no cause. The line ends at the
/// surface, and so does what can be caught on it.
///
/// `axes` says which of X, Y and Z are shown: an axis the user has turned off is
/// not on screen either.
pub fn axis_features(mesh: &Mesh, axes: [bool; 3]) -> Vec<Feature> {
    let Some((lo, hi)) = mesh.bounds() else { return Vec::new() };
    let mut out = Vec::new();
    for (axis, &shown) in axes.iter().enumerate() {
        if !shown {
            continue;
        }
        // The line is the axis itself, so it can only meet this body if the body
        // straddles zero on the other two coordinates.
        let others = [(axis + 1) % 3, (axis + 2) % 3];
        let range = |i: usize| (component(lo, i), component(hi, i));
        if others.iter().any(|&i| {
            let (l, h) = range(i);
            l > 1e-9 || h < -1e-9
        }) {
            continue;
        }
        let mut dir = Vec3::ZERO;
        set_component(&mut dir, axis, 1.0);

        // Every crossing along the whole line, in order, so entry and exit come
        // out as a pair.
        let mut hits: Vec<f64> = Vec::new();
        for tri in &mesh.indices {
            let (a, b, c) =
                (mesh.positions[tri[0] as usize], mesh.positions[tri[1] as usize], mesh.positions[tri[2] as usize]);
            if let Some(t) = line_triangle(Vec3::ZERO, dir, a, b, c) {
                hits.push(t);
            }
        }
        hits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        // A triangulated surface reports the shared edge of two triangles twice,
        // and a face the axis grazes reports a run of hits at one place.
        hits.dedup_by(|a, b| (*a - *b).abs() < 1e-6);

        out.extend(hits.iter().map(|&t| Feature::point(dir * t, FeatureKind::AxisCrossing)));
    }
    out
}

/// The lines the principal planes leave on one body's surface: the marks the
/// renderer draws on the solid itself, where each plane through the origin cuts
/// it, as segments in world space.
///
/// A mark is a chain of short segments, one per triangle the plane crosses --
/// exactly what is drawn, and exactly what a nearest-point search wants, since
/// the nearest point on a chain is the nearest point on one of its links. So
/// nothing joins them up.
///
/// `axes` says which of the three planes are marked: each follows the switch of
/// the axis it is perpendicular to, the same rule the drawing uses, so a plane
/// whose mark is not on screen is not caught either.
pub fn plane_mark_lines(mesh: &Mesh, axes: [bool; 3]) -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    for (axis, &shown) in axes.iter().enumerate() {
        if !shown {
            continue;
        }
        for tri in &mesh.indices {
            let world =
                [mesh.positions[tri[0] as usize], mesh.positions[tri[1] as usize], mesh.positions[tri[2] as usize]];
            if let Some(segment) = plane_crossing(world, axis) {
                out.push(segment);
            }
        }
    }
    out
}

/// Where the plane through the origin perpendicular to `axis` crosses one
/// triangle, as the segment it cuts. `None` when the triangle is wholly on one
/// side, which is nearly all of them, so this is the cheap case.
///
/// The renderer draws the marks and the measure tool catches them, and neither
/// may see a line the other does not, so both ask this.
pub fn plane_crossing(world: [Vec3; 3], axis: usize) -> Option<(Vec3, Vec3)> {
    let d = [component(world[0], axis), component(world[1], axis), component(world[2], axis)];
    if (d[0] > 0.0 && d[1] > 0.0 && d[2] > 0.0) || (d[0] < 0.0 && d[1] < 0.0 && d[2] < 0.0) {
        return None;
    }
    // A triangle lying *in* the plane has no crossing line of its own -- its
    // three edges are the mark, and its neighbours draw them.
    if d[0] == 0.0 && d[1] == 0.0 && d[2] == 0.0 {
        return None;
    }
    let mut hits: Vec<Vec3> = Vec::new();
    for i in 0..3 {
        let j = (i + 1) % 3;
        if d[i] == 0.0 {
            hits.push(world[i]);
        }
        if (d[i] < 0.0 && d[j] > 0.0) || (d[i] > 0.0 && d[j] < 0.0) {
            let t = d[i] / (d[i] - d[j]);
            hits.push(world[i] + (world[j] - world[i]) * t);
        }
    }
    (hits.len() >= 2).then(|| (hits[0], hits[1]))
}

fn component(v: Vec3, axis: usize) -> f64 {
    match axis {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

fn set_component(v: &mut Vec3, axis: usize, value: f64) {
    match axis {
        0 => v.x = value,
        1 => v.y = value,
        _ => v.z = value,
    }
}

/// Where an infinite line meets a triangle. The same intersection the picker
/// uses, without its "ahead of the origin only" rule: an axis runs both ways
/// from the origin and crosses bodies on both sides of it.
fn line_triangle(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f64> {
    let e1 = b - a;
    let e2 = c - a;
    let h = dir.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - a;
    let u = s.dot(h) * inv;
    if !(-1e-9..=1.0 + 1e-9).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return None;
    }
    Some(e2.dot(q) * inv)
}

/// The three world axes as segments long enough to cover the frame, for the
/// visible ones only.
///
/// An axis is a line, and a measurement along one -- "how far out along X is
/// this" -- wants to start anywhere on it, not only where it happens to meet a
/// body. `reach` is how far the axes are drawn from the origin, so nothing is
/// caught out where there is no line to see.
pub fn axis_lines(axes: [bool; 3], reach: f64) -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    for (axis, &shown) in axes.iter().enumerate() {
        if !shown {
            continue;
        }
        let mut dir = Vec3::ZERO;
        set_component(&mut dir, axis, reach);
        out.push((dir * -1.0, dir));
    }
    out
}

/// The nearest point on the segment `a`..`b` to `cursor`, measured on screen,
/// and how far away that landed (issue 78).
///
/// An edge is a line, not the three points on it the feature list carries, and
/// "measure from here along this edge" is an ordinary thing to want. The whole
/// segment is projected and the cursor is dropped onto it in screen space, so
/// what is caught is the place actually being pointed at. `None` when either end
/// falls off screen, where the projection cannot be trusted.
pub fn nearest_on_edge(
    a: Vec3,
    b: Vec3,
    project: impl Fn(Vec3) -> Option<egui::Pos2>,
    cursor: egui::Pos2,
    max_pixels: f32,
) -> Option<(Vec3, f32)> {
    let (sa, sb) = (project(a)?, project(b)?);
    let along = sb - sa;
    let length2 = along.length_sq();
    if length2 < 1e-9 {
        return None;
    }
    let t = ((cursor - sa).dot(along) / length2).clamp(0.0, 1.0);
    let distance = (sa + along * t - cursor).length();
    if distance > max_pixels {
        return None;
    }
    // The screen parameter is used in the model: a perspective divide would put
    // the point slightly off along the edge, but the viewport is orthographic
    // (issue 26), where the two parameters are the same number.
    Some((a + (b - a) * t as f64, distance))
}

/// Every feature within `max_pixels` of the cursor on screen, with how far away
/// each landed, in the order the list holds them.
///
/// The match is by screen distance, not world distance: what a user means by
/// "that corner" is the one under the pointer, and two corners far apart in the
/// model can sit close together in the frame. `project` returns `None` for a
/// point that does not land on screen, which is skipped.
///
/// All of them, rather than only the nearest, because a caller that will not
/// take every feature -- the measure tool, which takes only what the picture
/// shows -- has to work outward from the cursor until one is acceptable. Asking
/// that question of every feature instead costs a ray cast each.
pub fn near_on_screen(
    features: &[Feature],
    project: impl Fn(Vec3) -> Option<egui::Pos2>,
    cursor: egui::Pos2,
    max_pixels: f32,
) -> Vec<(&Feature, f32)> {
    let mut out = Vec::new();
    for feature in features {
        let Some(screen) = project(feature.point) else { continue };
        let distance = (screen - cursor).length();
        if distance <= max_pixels {
            out.push((feature, distance));
        }
    }
    out
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
        assert!(
            has_point(&features, FeatureKind::FaceCentre, Vec3::new(0.0, 0.0, 5.0)),
            "the top face centre is missing"
        );
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
        assert!(
            has_point(&features, FeatureKind::FaceCentre, Vec3::new(10.0, 0.0, 0.0)),
            "the +X face centre is missing"
        );
    }

    #[test]
    fn the_features_on_screen_are_the_ones_under_the_cursor_within_reach() {
        let features = vec![
            Feature::point(Vec3::new(0.0, 0.0, 0.0), FeatureKind::Vertex),
            Feature::point(Vec3::new(100.0, 0.0, 0.0), FeatureKind::Vertex),
            Feature::point(Vec3::new(6.0, 0.0, 0.0), FeatureKind::FaceCentre),
        ];
        // A trivial orthographic-ish projection: X and Y straight to screen.
        let project = |p: Vec3| Some(egui::pos2(p.x as f32, p.y as f32));

        // Both of the two in reach come back, each with how far off it is, so a
        // caller that will not take the nearest can work outward from it.
        let cursor = egui::pos2(3.0, 2.0);
        let found = near_on_screen(&features, project, cursor, 10.0);
        assert_eq!(found.len(), 2, "the far corner was within reach: {found:?}");
        let (nearest, distance) = found.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        assert_eq!(nearest.point, Vec3::ZERO);
        assert!((distance - (3.0f32 * 3.0 + 2.0 * 2.0).sqrt()).abs() < 1e-4);

        // Nothing within reach returns nothing, which is how a drag knows to fall
        // back to the grid.
        assert!(near_on_screen(&features, project, egui::pos2(50.0, 50.0), 10.0).is_empty());
    }

    #[test]
    fn an_edge_feature_carries_the_edge_it_is_the_middle_of() {
        // Issue 78: catching an edge anywhere along it needs its two ends, not
        // just the midpoint the feature reports.
        let mesh = primitives::box_mesh(10.0, 10.0, 10.0);
        let features = features_of(&mesh);
        let edges: Vec<&Feature> = features.iter().filter(|f| f.kind == FeatureKind::EdgeMidpoint).collect();
        assert_eq!(edges.len(), 12);
        for edge in edges {
            let (a, b) = edge.span.expect("an edge midpoint without its edge");
            assert!(((a + b) * 0.5 - edge.point).length() < 1e-9, "the midpoint is not the middle of its span");
            assert!((a - b).length() > 1.0, "a degenerate edge");
        }
        // The other kinds are points and nothing more.
        assert!(features.iter().filter(|f| f.kind != FeatureKind::EdgeMidpoint).all(|f| f.span.is_none()));
    }

    #[test]
    fn a_point_part_way_along_an_edge_is_caught_at_the_place_pointed_at() {
        let project = |p: Vec3| Some(egui::pos2(p.x as f32, p.y as f32));
        let (a, b) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 0.0));

        // A quarter of the way along, three pixels off the line: the caught point
        // is the one under the pointer, not either end and not the midpoint.
        let (at, distance) = nearest_on_edge(a, b, project, egui::pos2(25.0, 3.0), 10.0).unwrap();
        assert!((at - Vec3::new(25.0, 0.0, 0.0)).length() < 1e-6, "caught {at:?}");
        assert!((distance - 3.0).abs() < 1e-4);

        // Past an end it clamps to that end rather than running off the edge.
        let (at, _) = nearest_on_edge(a, b, project, egui::pos2(-40.0, 0.0), 100.0).unwrap();
        assert!((at - a).length() < 1e-6, "the catch ran off the end of the edge");

        // Out of reach across the line catches nothing.
        assert!(nearest_on_edge(a, b, project, egui::pos2(25.0, 40.0), 10.0).is_none());
    }

    #[test]
    fn the_axes_cross_a_body_at_points_that_can_be_caught() {
        // Issue 78: a box straddling the origin is crossed by all three axes, and
        // each crossing is a corner with the run between a pair as an edge.
        let mesh = primitives::box_mesh(20.0, 10.0, 6.0);
        let features = axis_features(&mesh, [true, true, true]);

        let crossings: Vec<&Feature> = features.iter().filter(|f| f.kind == FeatureKind::AxisCrossing).collect();
        assert_eq!(crossings.len(), 6, "three axes in and out of the box: {crossings:?}");
        for expected in [
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(-10.0, 0.0, 0.0),
            Vec3::new(0.0, 5.0, 0.0),
            Vec3::new(0.0, -5.0, 0.0),
            Vec3::new(0.0, 0.0, 3.0),
            Vec3::new(0.0, 0.0, -3.0),
        ] {
            assert!(
                crossings.iter().any(|f| (f.point - expected).length() < 1e-6),
                "no crossing at {expected:?} among {crossings:?}"
            );
        }

        // The crossings and nothing else: the run between them is inside the
        // material, where the line is cut out of the drawing, so there is
        // nothing there to catch.
        assert_eq!(features.len(), crossings.len(), "the stretch inside the body was offered as a snap target");
    }

    #[test]
    fn a_plane_cuts_a_triangle_in_at_most_one_segment() {
        let above = [Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 2.0), Vec3::new(0.0, 1.0, 3.0)];
        assert!(plane_crossing(above, 2).is_none());
        let crossing = [Vec3::new(0.0, 0.0, -1.0), Vec3::new(2.0, 0.0, 1.0), Vec3::new(0.0, 2.0, 1.0)];
        let (a, b) = plane_crossing(crossing, 2).expect("this triangle straddles z = 0");
        assert!(a.z.abs() < 1e-9 && b.z.abs() < 1e-9, "the cut has to lie in the plane: {a:?} {b:?}");
        // A triangle lying in the plane is left to its neighbours: its own
        // edges are the mark, and it has no interior crossing.
        let flat = [Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)];
        assert!(plane_crossing(flat, 2).is_none());
    }

    #[test]
    fn a_body_the_principal_planes_cut_carries_their_marks_as_lines() {
        // The mark is what the renderer draws on the solid, so what can be
        // caught is what is on screen: a segment per triangle the plane cuts,
        // every one of them lying in that plane.
        let mesh = primitives::box_mesh(20.0, 10.0, 6.0);
        for axis in 0..3 {
            let mut axes = [false; 3];
            axes[axis] = true;
            let lines = plane_mark_lines(&mesh, axes);
            assert!(!lines.is_empty(), "the plane perpendicular to axis {axis} cuts this box and marked nothing");
            for (a, b) in &lines {
                assert!(
                    component(*a, axis).abs() < 1e-9 && component(*b, axis).abs() < 1e-9,
                    "a mark off its own plane: {a:?} {b:?}"
                );
                assert!((*a - *b).length() > 1e-9, "a mark of no length");
            }
        }
        // All three at once is all three marks, and none of them without a plane
        // switched on.
        assert_eq!(
            plane_mark_lines(&mesh, [true; 3]).len(),
            (0..3).map(|axis| plane_mark_lines(&mesh, [axis == 0, axis == 1, axis == 2]).len()).sum::<usize>()
        );
        assert!(plane_mark_lines(&mesh, [false; 3]).is_empty());

        // A body no plane reaches has no mark on it, rather than a line hanging
        // in the air beside it.
        let away = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(100.0, 100.0, 100.0));
        assert!(plane_mark_lines(&away, [true; 3]).is_empty());
    }

    #[test]
    fn the_shown_axes_are_offered_as_lines_to_catch() {
        let lines = axis_lines([true, true, true], 500.0);
        assert_eq!(lines.len(), 3);
        for (i, (a, b)) in lines.iter().enumerate() {
            // Each runs through the origin, both ways, along one axis only.
            assert!(((*a + *b) * 0.5).length() < 1e-9, "axis {i} is not centred on the origin");
            assert!((a.length() - 500.0).abs() < 1e-9 && (b.length() - 500.0).abs() < 1e-9);
        }
        assert_eq!(
            axis_lines([false, true, false], 10.0),
            vec![(Vec3::new(0.0, -10.0, 0.0), Vec3::new(0.0, 10.0, 0.0))]
        );
        assert!(axis_lines([false; 3], 10.0).is_empty());
    }

    #[test]
    fn an_axis_that_is_turned_off_or_misses_the_body_offers_nothing() {
        let mesh = primitives::box_mesh(20.0, 10.0, 6.0);
        // Only Z is shown: only Z's two crossings and its one run.
        let features = axis_features(&mesh, [false, false, true]);
        assert_eq!(features.iter().filter(|f| f.kind == FeatureKind::AxisCrossing).count(), 2);
        assert!(features.iter().all(|f| f.point.x.abs() < 1e-9 && f.point.y.abs() < 1e-9));
        assert!(axis_features(&mesh, [false; 3]).is_empty());

        // A body the axes miss entirely has no crossings, rather than points
        // conjured somewhere near it.
        let away = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(100.0, 100.0, 100.0));
        assert!(axis_features(&away, [true, true, true]).is_empty());
    }
}
