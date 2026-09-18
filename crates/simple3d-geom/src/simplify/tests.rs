//! What a simplification is allowed to do to a shape, and what it is not.

use super::*;
use crate::primitives::{ellipsoid_mesh, plate_mesh};
use crate::vec3::Vec3;

fn sphere() -> Mesh {
    ellipsoid_mesh(20.0, 20.0, 20.0, 24)
}

fn assert_sound(name: &str, mesh: &Mesh) {
    assert!(mesh.triangle_count() >= MIN_TRIANGLES, "{name}: {} triangles left", mesh.triangle_count());
    if let Some(issue) = mesh.manifold_issue() {
        panic!("{name}: the result is not a closed surface: {issue}");
    }
}

/// How far the simplified surface ended up from the original, worked out the
/// slow and obvious way: every vertex of the original against every triangle of
/// the result.
///
/// The run reports the same number off a grid, and the point of measuring it
/// twice is that the grid is an optimisation -- a cell size, a ring search and
/// a rule for triangles too big to bucket -- and an optimisation that quietly
/// misses the nearest triangle would make the whole deviation figure a
/// comforting lie. This is the version with nothing in it to get wrong.
fn exact_deviation(original: &Mesh, result: &Mesh) -> f64 {
    original
        .positions
        .iter()
        .map(|&p| {
            result
                .indices
                .iter()
                .map(|tri| {
                    let points = tri.map(|v| result.positions[v as usize]);
                    point_to_triangle(p, points)
                })
                .fold(f64::MAX, f64::min)
        })
        .fold(0.0, f64::max)
}

fn point_to_triangle(p: Vec3, [a, b, c]: [Vec3; 3]) -> f64 {
    let normal = (b - a).cross(c - a);
    let area = normal.length();
    if area > 0.0 {
        let n = normal * (1.0 / area);
        let on_plane = p - n * (p - a).dot(n);
        if [(a, b), (b, c), (c, a)].iter().all(|&(from, to)| (to - from).cross(on_plane - from).dot(n) >= 0.0) {
            return (p - on_plane).length();
        }
    }
    [(a, b), (b, c), (c, a)]
        .iter()
        .map(|&(from, to)| {
            let along = to - from;
            let length = along.dot(along);
            let t = if length > 0.0 { ((p - from).dot(along) / length).clamp(0.0, 1.0) } else { 0.0 };
            (p - (from + along * t)).length()
        })
        .fold(f64::MAX, f64::min)
}

#[test]
fn drops_to_the_budget() {
    let before = sphere();
    let after = simplify(&before, &Simplify { detail: 25, keep_sharp: false, ..Simplify::default() });
    assert_sound("quarter sphere", &after.mesh);
    let target = before.triangle_count() / 4;
    assert!(
        after.mesh.triangle_count() <= target + target / 10,
        "asked for a quarter of {}, got {}",
        before.triangle_count(),
        after.mesh.triangle_count()
    );
}

#[test]
fn keeps_the_shape() {
    let before = sphere();
    let after = simplify(&before, &Simplify { detail: 20, keep_sharp: false, ..Simplify::default() });
    let (lo, hi) = after.mesh.bounds().expect("there is still a shape");
    let (was_lo, was_hi) = before.bounds().expect("there was a shape");
    // A sphere simplified to a fifth loses some of its bulge, but it stays a
    // sphere of about the same size: a result that has walked off its own
    // bounding box is a result that has gone wrong.
    for (now, was) in [(lo, was_lo), (hi, was_hi)] {
        for (now, was) in [(now.x, was.x), (now.y, was.y), (now.z, was.z)] {
            assert!((now - was).abs() < 2.0, "the bounds moved from {was} to {now}");
        }
    }
}

/// The budget spent on nothing: no collapse is made, and what comes back is the
/// mesh as it was.
///
/// As it was *welded*, which is not quite the same mesh a generator produces:
/// welding is the first thing every run does -- edges cannot be found on a
/// surface whose triangles do not share vertices -- and a sphere's poles are
/// degenerate triangles that welding takes out. That is not detail being
/// dropped; a mesh stored on a node has been through the same weld already.
#[test]
fn a_hundred_percent_changes_nothing() {
    let before = sphere();
    let after = simplify(&before, &Simplify { detail: 100, ..Simplify::default() });
    assert_eq!(after.mesh.triangle_count(), before.weld().triangle_count());
    assert_eq!(after.deviation, 0.0);
}

/// A box is twelve triangles and eight corners, and every one of its edges is a
/// right angle. There is no detail in it to drop, and a simplification that
/// "succeeded" on it would have taken a corner off the shape.
#[test]
fn a_box_has_nothing_to_drop() {
    let before = plate_mesh(40.0, 30.0, 10.0);
    let after = simplify(&before, &Simplify { detail: 10, ..Simplify::default() });
    assert_eq!(after.mesh.triangle_count(), before.triangle_count());
    assert_eq!(after.deviation, 0.0);
}

/// The same box with the creases no longer treated as features: now it can be
/// simplified, and what it comes back as is still a closed surface.
#[test]
fn a_box_gives_way_once_its_creases_are_not_kept() {
    let before = plate_mesh(40.0, 30.0, 10.0);
    let after = simplify(&before, &Simplify { detail: 50, keep_sharp: false, ..Simplify::default() });
    assert!(after.mesh.triangle_count() < before.triangle_count());
    assert_sound("box without creases", &after.mesh);
}

#[test]
fn the_deviation_cap_is_honoured() {
    let before = sphere();
    let plan =
        Simplify { detail: 5, keep_sharp: false, limit_deviation: true, max_deviation: 0.2, ..Simplify::default() };
    let after = simplify(&before, &plan);
    assert_sound("capped sphere", &after.mesh);
    assert!(after.deviation <= 0.2, "reported {} against a cap of 0.2", after.deviation);
    assert!(exact_deviation(&before.weld(), &after.mesh) <= 0.2, "the surface ended up outside the cap");
    // A cap that tight stops the run long before the budget does, which is the
    // whole point of having it: the budget asked for a twentieth.
    assert!(after.mesh.triangle_count() > before.triangle_count() / 20);
}

/// The number the tool puts on screen is a measurement of the result, and this
/// is the measurement made again by hand.
#[test]
fn the_reported_deviation_is_what_the_surface_actually_did() {
    let before = sphere();
    for detail in [60u32, 30] {
        let after = simplify(&before, &Simplify { detail, keep_sharp: false, ..Simplify::default() });
        let exact = exact_deviation(&before.weld(), &after.mesh);
        assert!(
            (after.deviation - exact).abs() <= 1e-9,
            "at {detail}% it reported {} where the surface moved {exact}",
            after.deviation
        );
        assert!(exact > 0.0, "a simplification that moved nothing at all is not one");
    }
}

/// A tighter cap can only keep more of the mesh than a looser one.
#[test]
fn a_tighter_cap_keeps_more() {
    let before = sphere();
    let count = |deviation: f64| {
        let plan = Simplify {
            detail: 5,
            keep_sharp: false,
            limit_deviation: true,
            max_deviation: deviation,
            ..Simplify::default()
        };
        simplify(&before, &plan).mesh.triangle_count()
    };
    assert!(count(0.05) > count(0.5));
}

/// An open mesh -- one triangle's worth of surface with a rim -- keeps its rim
/// exactly where it was, so a part simplified beside another part still meets
/// it.
#[test]
fn a_boundary_is_kept() {
    let mut before = Mesh::new();
    let rows = 12;
    let step = 60.0 / f64::from(rows);
    let at = |x: i32, y: i32| Vec3::new(f64::from(x) * step, f64::from(y) * step, 0.0);
    for x in 0..rows {
        for y in 0..rows {
            before.push_triangle(at(x, y), at(x + 1, y), at(x + 1, y + 1));
            before.push_triangle(at(x, y), at(x + 1, y + 1), at(x, y + 1));
        }
    }
    let after = simplify(&before, &Simplify { detail: 20, keep_sharp: false, ..Simplify::default() });
    let (lo, hi) = after.mesh.bounds().expect("there is still a sheet");
    assert_eq!((lo.x, lo.y), (0.0, 0.0), "the rim moved in");
    assert_eq!((hi.x, hi.y), (60.0, 60.0), "the rim moved in");
}

/// Every vertex on the line between two differently painted surfaces, which is
/// the line a seam-keeping run may not move.
fn seam_points(mesh: &Mesh) -> Vec<Vec3> {
    let welded = mesh.weld();
    let mut along: std::collections::HashMap<(u32, u32), Vec<usize>> = std::collections::HashMap::new();
    for (index, tri) in welded.indices.iter().enumerate() {
        for i in 0..3 {
            let (a, b) = (tri[i], tri[(i + 1) % 3]);
            along.entry((a.min(b), a.max(b))).or_default().push(index);
        }
    }
    let mut points = Vec::new();
    for ((a, b), tris) in along {
        if tris.len() == 2 && welded.tag(tris[0]) != welded.tag(tris[1]) {
            points.push(welded.positions[a as usize]);
            points.push(welded.positions[b as usize]);
        }
    }
    points
}

/// Two colours on one mesh, and the line between them is not to be crossed. The
/// surface either side of it is simplified; the line itself comes back
/// vertex for vertex, in the same places, so the paint still ends where it
/// ended.
#[test]
fn a_colour_seam_survives() {
    let mut before = sphere();
    let painted = crate::colour_tag([200, 40, 40]);
    for (index, tri) in before.indices.iter().enumerate() {
        let above = before.positions[tri[0] as usize].z > 0.0;
        before.tags[index] = if above { painted } else { 0 };
    }
    let after = simplify(&before, &Simplify { detail: 20, keep_sharp: false, ..Simplify::default() });
    assert!(after.mesh.tags.contains(&painted), "the paint is gone");
    assert!(after.mesh.tags.contains(&0), "the unpainted surface is gone");
    assert!(after.mesh.triangle_count() < before.triangle_count(), "nothing was simplified at all");
    for point in seam_points(&before) {
        assert!(after.mesh.positions.contains(&point), "the seam moved: {point:?} is gone");
    }
}

/// Abandoning a run gives back nothing rather than a half-simplified mesh: a
/// partial answer is not an answer, and the caller that asked for it has
/// already moved on.
#[test]
fn an_abandoned_run_gives_nothing_back() {
    let before = sphere();
    let outcome = simplify_until(&before, &Simplify { detail: 10, keep_sharp: false, ..Simplify::default() }, &|| true);
    assert!(outcome.is_none());
}
