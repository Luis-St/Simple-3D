use crate::mesh::Mesh;
use crate::vec3::Vec3;

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
        let world = [mesh.positions[tri[0] as usize], mesh.positions[tri[1] as usize], mesh.positions[tri[2] as usize]];
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
