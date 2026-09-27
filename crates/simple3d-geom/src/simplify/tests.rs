//! What a simplification is allowed to do to a shape, and what it is not.

use super::*;
use crate::primitives::{ellipsoid_mesh, plate_mesh};
use crate::vec3::Vec3;
use measure::point_to_triangle;

fn sphere() -> Mesh {
    ellipsoid_mesh(20.0, 20.0, 20.0, 24)
}

fn assert_sound(name: &str, mesh: &Mesh) {
    assert!(mesh.triangle_count() >= MIN_TRIANGLES, "{name}: {} triangles left", mesh.triangle_count());
    if let Some(issue) = mesh.manifold_issue() {
        panic!("{name}: the result is not a closed surface: {issue}");
    }
}

/// The deviation computed the slow, obvious way (every original vertex against every result
/// triangle), to check the run's grid-based figure.
fn exact_deviation(original: &Mesh, result: &Mesh) -> f64 {
    original
        .positions
        .iter()
        .map(|&p| {
            result
                .indices
                .iter()
                .map(|tri| {
                    let points = result.corners(*tri);
                    point_to_triangle(p, points)
                })
                .fold(f64::MAX, f64::min)
        })
        .fold(0.0, f64::max)
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
    // A simplified sphere stays about the same size; leaving its bounding box means something broke.
    for (now, was) in [(lo, was_lo), (hi, was_hi)] {
        for (now, was) in [(now.x, was.x), (now.y, was.y), (now.z, was.z)] {
            assert!((now - was).abs() < 2.0, "the bounds moved from {was} to {now}");
        }
    }
}

/// With no budget spent, the mesh comes back as welded. Welding (which drops the poles' degenerate
/// triangles) always happens first, and stored meshes are already welded.
#[test]
fn a_hundred_percent_changes_nothing() {
    let before = sphere();
    let after = simplify(&before, &Simplify { detail: 100, ..Simplify::default() });
    assert_eq!(after.mesh.triangle_count(), before.weld().triangle_count());
    assert_eq!(after.deviation, 0.0);
}

/// A box has no detail to drop: every edge is a right angle.
#[test]
fn a_box_has_nothing_to_drop() {
    let before = plate_mesh(40.0, 30.0, 10.0);
    let after = simplify(&before, &Simplify { detail: 10, ..Simplify::default() });
    assert_eq!(after.mesh.triangle_count(), before.triangle_count());
    assert_eq!(after.deviation, 0.0);
}

/// Without crease protection the box simplifies and stays closed.
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
    // The tight cap stops the run long before the budget (a twentieth) does.
    assert!(after.mesh.triangle_count() > before.triangle_count() / 20);
}

/// The deviation the tool shows matches an independent measurement.
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

/// An open mesh keeps its rim exactly, so neighbouring parts still meet.
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

/// Every vertex on the line between two differently painted surfaces.
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

/// A colour seam is kept vertex for vertex while either side is simplified.
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

/// An abandoned run returns nothing rather than a half-simplified mesh.
#[test]
fn an_abandoned_run_gives_nothing_back() {
    let before = sphere();
    let outcome = simplify_until(&before, &Simplify { detail: 10, keep_sharp: false, ..Simplify::default() }, &|| true);
    assert!(outcome.is_none());
}
