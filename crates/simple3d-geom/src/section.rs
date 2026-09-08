//! Cutting the model with a plane so its inside can be seen (issue 71).
//!
//! Nothing here changes a model. A section is a way of *looking* at one: the
//! material on the far side of a plane is left out of the picture, and the
//! opening that leaves is closed with a cap, so a wall reads as a wall with a
//! thickness rather than as a hollow shell seen from inside.
//!
//! The two halves of that are here because both are geometry and both have to
//! answer the same way. [`clip_triangle`] is what the renderer walks the model
//! with, one triangle at a time; [`loops`] finds the closed outlines the plane
//! leaves in a mesh and [`cap`] fills them. The outlines are worth having on
//! their own -- they are a 2D section of the model, in world space -- which is
//! what pulling a drawing back out of a cut would start from.

use crate::mesh::Mesh;
use crate::planar;
use crate::vec3::Vec3;
use std::collections::HashMap;

/// The plane the model is cut with, as a half-space: what lies on the `normal`
/// side of it is not drawn.
///
/// `normal` points at the material that goes away, so a plane with a `+Z`
/// normal takes the top off. It need not be a unit vector for the sign tests to
/// work, but [`cap`] and the winding rule below take directions from it, so
/// [`Plane::new`] normalises it once and everything downstream can rely on that.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    pub normal: Vec3,
    /// Where along the normal the plane sits, in millimetres.
    pub offset: f64,
}

impl Plane {
    pub fn new(normal: Vec3, offset: f64) -> Plane {
        Plane { normal: normal.normalized(), offset }
    }

    /// The plane perpendicular to one of the three axes, `offset` along it.
    pub fn on_axis(axis: usize, offset: f64, flipped: bool) -> Plane {
        let normal = match axis {
            0 => Vec3::new(1.0, 0.0, 0.0),
            1 => Vec3::new(0.0, 1.0, 0.0),
            _ => Vec3::new(0.0, 0.0, 1.0),
        };
        // Flipping keeps the plane where it is and swaps which side of it
        // survives, so the offset is negated with the normal.
        match flipped {
            true => Plane { normal: -normal, offset: -offset },
            false => Plane { normal, offset },
        }
    }

    /// How far past the plane `p` is: negative on the side that is kept, zero
    /// on the plane itself.
    pub fn depth(&self, p: Vec3) -> f64 {
        self.normal.dot(p) - self.offset
    }

    pub fn keeps(&self, p: Vec3) -> bool {
        self.depth(p) <= 0.0
    }

    /// A point on the plane: the foot of the normal from the origin. What the
    /// interface hangs the plane's own frame and its grip on.
    pub fn origin(&self) -> Vec3 {
        self.normal * self.offset
    }
}

/// What is left of one triangle once the plane has had it, and the edge the cut
/// left behind.
///
/// At most two triangles: a triangle with one corner on the kept side comes
/// back as itself shrunk, and one with two comes back as a quad, which is two.
/// They are held in an array rather than a `Vec` because this is called once
/// per triangle of the whole scene on every frame the picture changes, and an
/// allocation there is the difference between a section costing nothing and
/// costing the frame.
pub struct Clipped {
    triangles: [[Vec3; 3]; 2],
    count: usize,
    /// The cut edge, wound for the *cap* -- see [`loops`] for what that means
    /// and why the direction matters.
    pub cut: Option<[Vec3; 2]>,
}

impl Clipped {
    /// A triangle no section touched, so that a caller drawing with the section
    /// off and one drawing with it on can walk the same answer.
    pub fn untouched(world: [Vec3; 3]) -> Clipped {
        Clipped::whole(world)
    }

    pub fn triangles(&self) -> &[[Vec3; 3]] {
        &self.triangles[..self.count]
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn nothing() -> Clipped {
        Clipped { triangles: [[Vec3::ZERO; 3]; 2], count: 0, cut: None }
    }

    fn whole(world: [Vec3; 3]) -> Clipped {
        Clipped { triangles: [world, [Vec3::ZERO; 3]], count: 1, cut: None }
    }
}

/// The part of `world` on the kept side of `plane`.
///
/// The usual answer is "all of it" or "none of it" -- a plane crosses a band of
/// triangles and misses every other one -- so both of those are decided on
/// three comparisons before anything is worked out.
pub fn clip_triangle(plane: &Plane, world: [Vec3; 3]) -> Clipped {
    let d = [plane.depth(world[0]), plane.depth(world[1]), plane.depth(world[2])];
    // Wholly kept, which includes a triangle lying *in* the plane: it is the
    // surface the cut runs along, not something the cut removes.
    if d.iter().all(|&at| at <= 0.0) {
        return Clipped::whole(world);
    }
    if d.iter().all(|&at| at >= 0.0) {
        return Clipped::nothing();
    }

    // Sutherland-Hodgman against the one plane, in the triangle's own winding
    // so the polygon that comes out keeps the surface's orientation.
    let mut poly = [Vec3::ZERO; 4];
    let mut corners = 0;
    // Where the cut meets this triangle: the crossings of its edges, plus any
    // corner sitting exactly on the plane -- a plane through a box's own
    // vertices is the common case, not the exotic one, and a crossing counted
    // only where a *strict* sign change happens misses those.
    let mut hits: [Vec3; 3] = [Vec3::ZERO; 3];
    let mut hit_count = 0;
    for i in 0..3 {
        let j = (i + 1) % 3;
        if d[i] <= 0.0 {
            poly[corners] = world[i];
            corners += 1;
        }
        if d[i] == 0.0 && hit_count < 3 {
            hits[hit_count] = world[i];
            hit_count += 1;
        }
        if (d[i] < 0.0 && d[j] > 0.0) || (d[i] > 0.0 && d[j] < 0.0) {
            let t = d[i] / (d[i] - d[j]);
            let at = world[i] + (world[j] - world[i]) * t;
            poly[corners] = at;
            corners += 1;
            if hit_count < 3 {
                hits[hit_count] = at;
                hit_count += 1;
            }
        }
    }

    let mut out = Clipped::nothing();
    if corners >= 3 {
        out.triangles[0] = [poly[0], poly[1], poly[2]];
        out.count = 1;
        if corners == 4 {
            out.triangles[1] = [poly[0], poly[2], poly[3]];
            out.count = 2;
        }
        out.cut = cap_edge(world, &hits[..hit_count], plane);
    }
    out
}

/// The cut edge of one clipped triangle, wound the way the cap wants it.
///
/// The direction is not a guess. The cap closes the kept solid, so along their
/// shared edge it runs *against* the face the edge came off -- that is what
/// makes a closed surface closed. Walking the clipped face in its own winding,
/// the cut edge runs along `n x m` (the face's outward normal crossed with the
/// plane's), so the cap's runs along `m x n`. Every cap edge taken that way
/// chains into loops that are counter-clockwise about the plane normal for an
/// outer boundary and clockwise for a hole, which is exactly what the
/// triangulator reads them as.
fn cap_edge(world: [Vec3; 3], hits: &[Vec3], plane: &Plane) -> Option<[Vec3; 2]> {
    if hits.len() < 2 {
        return None;
    }
    let (from, to) = (hits[0], hits[hits.len() - 1]);
    if (to - from).length() < 1e-12 {
        return None;
    }
    let normal = (world[1] - world[0]).cross(world[2] - world[0]);
    let forward = plane.normal.cross(normal);
    // A face lying in the plane has no direction to take -- and no cut edge of
    // its own either: its neighbours draw the boundary around it.
    if forward.length() < 1e-12 {
        return None;
    }
    match (to - from).dot(forward) >= 0.0 {
        true => Some([from, to]),
        false => Some([to, from]),
    }
}

/// The part of the segment `a`-`b` on the kept side, or `None` when the whole
/// of it is cut away. What every *line* of the picture goes through: a feature
/// edge, a selection outline, the mark a principal plane leaves on a surface.
pub fn clip_segment(plane: &Plane, a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {
    let (da, db) = (plane.depth(a), plane.depth(b));
    if da <= 0.0 && db <= 0.0 {
        return Some((a, b));
    }
    if da > 0.0 && db > 0.0 {
        return None;
    }
    let at = a + (b - a) * (da / (da - db));
    match da <= 0.0 {
        true => Some((a, at)),
        false => Some((at, b)),
    }
}

/// The closed outlines the plane leaves in `mesh`: the shape of the material
/// where it was cut, in world space.
///
/// Each loop is wound counter-clockwise about the plane normal seen from the
/// side that was cut away, and a hole in the material the other way round --
/// the convention a filled outline is read with everywhere in this crate.
///
/// `mesh` should be welded, so that the two triangles sharing an edge compute
/// the same crossing point on it and the segments chain up. A stretch that does
/// not close is dropped rather than guessed at: the cap it would have made is
/// worth less than a wrong one is harmful.
pub fn loops(mesh: &Mesh, plane: &Plane) -> Vec<Vec<Vec3>> {
    let mut segments: Vec<[Vec3; 2]> = Vec::new();
    for tri in &mesh.indices {
        let world = [mesh.positions[tri[0] as usize], mesh.positions[tri[1] as usize], mesh.positions[tri[2] as usize]];
        if let Some(cut) = clip_triangle(plane, world).cut {
            segments.push(cut);
        }
    }
    chain(&segments)
}

/// Buckets a point falls in for the purpose of joining segments end to end, at
/// the same 1e-6 mm the mesh welder uses: two triangles either side of an edge
/// cross the plane at the same place, but not always in the same last bit.
fn key(p: Vec3) -> (i64, i64, i64) {
    let s = 1_000_000.0;
    ((p.x * s).round() as i64, (p.y * s).round() as i64, (p.z * s).round() as i64)
}

/// Join the cut edges into closed loops, following each one from its end to
/// whichever edge starts there.
fn chain(segments: &[[Vec3; 2]]) -> Vec<Vec<Vec3>> {
    let mut starting: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (index, segment) in segments.iter().enumerate() {
        starting.entry(key(segment[0])).or_default().push(index);
    }
    let mut used = vec![false; segments.len()];
    let mut out = Vec::new();
    for first in 0..segments.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let home = key(segments[first][0]);
        let mut points = vec![segments[first][0]];
        let mut at = segments[first][1];
        // A loop cannot be longer than the number of edges there are, and
        // saying so is what keeps a mesh that chains into a knot from spinning
        // here for ever.
        for _ in 0..segments.len() {
            if key(at) == home {
                break;
            }
            let Some(next) = starting.get(&key(at)).and_then(|from| from.iter().copied().find(|&i| !used[i])) else {
                points.clear();
                break;
            };
            used[next] = true;
            points.push(segments[next][0]);
            at = segments[next][1];
        }
        // Anything that did not come back to where it started is not a loop,
        // and a loop of two points encloses nothing.
        if points.len() >= 3 && key(at) == home {
            out.push(points);
        }
    }
    out
}

/// The cap: the cut filled in, as triangles facing the side that was cut away.
///
/// Holes come out as holes -- the triangulator reads a loop wound against the
/// normal as one -- so the cap of a tube is a ring and a section through it
/// shows the wall it actually has. An outline the triangulator cannot make
/// sense of contributes nothing rather than a guess, which leaves that part of
/// the cut open: the picture is then what it was before caps existed, which is
/// a fair way to fail.
pub fn cap(mesh: &Mesh, plane: &Plane) -> Vec<[Vec3; 3]> {
    fill(&loops(mesh, plane), plane.normal)
}

/// Fill closed outlines that all lie in one plane. Split out so a caller that
/// already has the outlines -- the renderer draws them as well as fills them --
/// does not find them twice.
pub fn fill(outlines: &[Vec<Vec3>], normal: Vec3) -> Vec<[Vec3; 3]> {
    if outlines.is_empty() {
        return Vec::new();
    }
    let mut positions: Vec<Vec3> = Vec::new();
    let mut indexed: Vec<Vec<u32>> = Vec::new();
    for outline in outlines {
        let start = positions.len() as u32;
        positions.extend(outline.iter().copied());
        indexed.push((0..outline.len() as u32).map(|i| start + i).collect());
    }
    match planar::triangulate_loops(&positions, normal, indexed) {
        Some(triangles) => triangles
            .into_iter()
            .map(|t| [positions[t[0] as usize], positions[t[1] as usize], positions[t[2] as usize]])
            .collect(),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// The area of a set of triangles, which is how a cap is checked: what it
    /// covers is the question, not how it happens to be triangulated.
    fn area(triangles: &[[Vec3; 3]]) -> f64 {
        triangles.iter().map(|t| (t[1] - t[0]).cross(t[2] - t[0]).length() * 0.5).sum()
    }

    fn box_mesh() -> Mesh {
        primitives::box_mesh(20.0, 20.0, 20.0).weld()
    }

    #[test]
    fn a_triangle_wholly_on_either_side_is_kept_or_dropped_whole() {
        let plane = Plane::new(Vec3::new(0.0, 0.0, 1.0), 0.0);
        let below = [Vec3::new(0.0, 0.0, -1.0), Vec3::new(1.0, 0.0, -1.0), Vec3::new(0.0, 1.0, -2.0)];
        let kept = clip_triangle(&plane, below);
        assert_eq!(kept.triangles(), [below], "a triangle on the kept side comes back untouched");
        assert!(kept.cut.is_none(), "nothing was cut, so there is no cut edge");

        let above = [Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 1.0), Vec3::new(0.0, 1.0, 2.0)];
        assert!(clip_triangle(&plane, above).is_empty());
    }

    #[test]
    fn clipping_a_triangle_leaves_the_part_on_the_kept_side() {
        let plane = Plane::new(Vec3::new(1.0, 0.0, 0.0), 5.0);
        // A right triangle of area 50 in the z = 0 plane, cut at x = 5: the
        // kept part is the trapezium from x = 0 to x = 5.
        let tri = [Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), Vec3::new(0.0, 10.0, 0.0)];
        let clipped = clip_triangle(&plane, tri);
        assert_eq!(clipped.triangles().len(), 2, "a triangle with two corners kept is a quad");
        assert!((area(clipped.triangles()) - 37.5).abs() < 1e-9, "kept {}", area(clipped.triangles()));
        let cut = clipped.cut.expect("the plane crossed the triangle");
        assert!(cut.iter().all(|p| (p.x - 5.0).abs() < 1e-9), "the cut edge is not in the plane: {cut:?}");
    }

    #[test]
    fn a_corner_exactly_on_the_plane_still_gives_a_cut_edge() {
        // The case a strict sign change misses, and the one an axis-aligned
        // plane through a box's own vertices lands in constantly.
        let plane = Plane::new(Vec3::new(1.0, 0.0, 0.0), 0.0);
        let tri = [Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), Vec3::new(-10.0, 10.0, 0.0)];
        let cut = clip_triangle(&plane, tri).cut.expect("one corner sits on the plane and the far one crosses it");
        assert!(cut.iter().any(|p| p.length() < 1e-9), "the corner on the plane is an end of the cut edge");
    }

    #[test]
    fn a_segment_is_trimmed_at_the_plane() {
        let plane = Plane::new(Vec3::new(0.0, 0.0, 1.0), 2.0);
        let (a, b) = clip_segment(&plane, Vec3::new(0.0, 0.0, -4.0), Vec3::new(0.0, 0.0, 6.0)).expect("it crosses");
        assert_eq!(a, Vec3::new(0.0, 0.0, -4.0));
        assert!((b.z - 2.0).abs() < 1e-9, "trimmed to {b:?}");
        // The far side of the same segment, whichever way round it is given.
        let (a, b) = clip_segment(&plane, Vec3::new(0.0, 0.0, 6.0), Vec3::new(0.0, 0.0, -4.0)).expect("it crosses");
        assert!((a.z - 2.0).abs() < 1e-9, "trimmed to {a:?}");
        assert_eq!(b, Vec3::new(0.0, 0.0, -4.0));
        assert!(clip_segment(&plane, Vec3::new(0.0, 0.0, 3.0), Vec3::new(0.0, 0.0, 9.0)).is_none());
    }

    #[test]
    fn a_box_cut_through_the_middle_caps_with_its_own_cross_section() {
        let plane = Plane::new(Vec3::new(0.0, 0.0, 1.0), 0.0);
        let outlines = loops(&box_mesh(), &plane);
        assert_eq!(outlines.len(), 1, "a solid box has one outline at any height");
        let filled = cap(&box_mesh(), &plane);
        assert!(!filled.is_empty(), "the cut was left open");
        assert!((area(&filled) - 400.0).abs() < 1e-6, "the cap covers {} of 400", area(&filled));
    }

    #[test]
    fn the_cap_faces_the_side_that_was_cut_away() {
        // Which way the cap faces is the whole of the winding rule, and getting
        // it backwards is invisible in an area check.
        let plane = Plane::new(Vec3::new(0.0, 0.0, 1.0), 0.0);
        for triangle in cap(&box_mesh(), &plane) {
            let normal = (triangle[1] - triangle[0]).cross(triangle[2] - triangle[0]).normalized();
            assert!(normal.z > 0.9, "a cap triangle faces {normal:?}, not the way the material went");
        }
    }

    #[test]
    fn a_tube_is_capped_as_a_ring_so_its_wall_can_be_measured() {
        // The case the feature exists for: the cut has to show a wall, which
        // means the outline inside it has to come back as a hole.
        let outer = 20.0;
        let inner = 14.0;
        let tube = primitives::tube_mesh(outer, inner, 30.0, 64).weld();
        let plane = Plane::new(Vec3::new(0.0, 0.0, 1.0), 0.0);
        let outlines = loops(&tube, &plane);
        assert_eq!(outlines.len(), 2, "a tube's cut is an outer outline and a hole");
        let filled = cap(&tube, &plane);
        let expected = std::f64::consts::PI * ((outer / 2.0).powi(2) - (inner / 2.0).powi(2));
        // A 64-segment circle is a polygon, so the area is a little under the
        // circle's: within a percent is the tessellation, not a hole in the cap.
        let covered = area(&filled);
        assert!(covered < expected && covered > expected * 0.99, "the ring covers {covered} of about {expected}");
    }

    #[test]
    fn a_plane_that_misses_the_model_cuts_nothing() {
        let plane = Plane::new(Vec3::new(0.0, 0.0, 1.0), 500.0);
        assert!(loops(&box_mesh(), &plane).is_empty());
        assert!(cap(&box_mesh(), &plane).is_empty());
    }

    #[test]
    fn a_plane_exactly_on_a_face_of_the_model_leaves_it_whole_and_uncapped() {
        // Cutting a 20mm box at z = 10 takes nothing off it: every triangle is
        // on the kept side or in the plane, and a cap over a face that is
        // already there would only fight with it for the pixels.
        let plane = Plane::new(Vec3::new(0.0, 0.0, 1.0), 10.0);
        let mesh = box_mesh();
        assert!(loops(&mesh, &plane).is_empty(), "a cut that removed nothing produced an outline");
        for tri in &mesh.indices {
            let world =
                [mesh.positions[tri[0] as usize], mesh.positions[tri[1] as usize], mesh.positions[tri[2] as usize]];
            assert_eq!(clip_triangle(&plane, world).triangles().len(), 1, "a triangle was cut where nothing was");
        }
    }

    #[test]
    fn two_separate_bodies_are_each_capped() {
        let mut mesh = primitives::box_mesh(10.0, 10.0, 10.0);
        mesh.append(&primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(40.0, 0.0, 0.0)));
        let mesh = mesh.weld();
        let plane = Plane::new(Vec3::new(0.0, 0.0, 1.0), 0.0);
        assert_eq!(loops(&mesh, &plane).len(), 2, "one outline per body");
        assert!((area(&cap(&mesh, &plane)) - 200.0).abs() < 1e-6);
    }
}
