use crate::mesh::Mesh;
use crate::vec3::Vec3;

use super::*;
use crate::primitives;

/// The area of a set of triangles, since a cap is judged by what it covers, not its triangulation.
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
    // A right triangle of area 50 in z = 0, cut at x = 5: the trapezium from x = 0 to 5 is kept.
    let tri = [Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), Vec3::new(0.0, 10.0, 0.0)];
    let clipped = clip_triangle(&plane, tri);
    assert_eq!(clipped.triangles().len(), 2, "a triangle with two corners kept is a quad");
    assert!((area(clipped.triangles()) - 37.5).abs() < 1e-9, "kept {}", area(clipped.triangles()));
    let cut = clipped.cut.expect("the plane crossed the triangle");
    assert!(cut.iter().all(|p| (p.x - 5.0).abs() < 1e-9), "the cut edge is not in the plane: {cut:?}");
}

#[test]
fn a_corner_exactly_on_the_plane_still_gives_a_cut_edge() {
    // The case a strict sign change misses, common for planes through a box's vertices.
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
    // The far side of the same segment, given either way round.
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
    // The cap's facing is the winding rule, invisible to an area check.
    let plane = Plane::new(Vec3::new(0.0, 0.0, 1.0), 0.0);
    for triangle in cap(&box_mesh(), &plane) {
        let normal = (triangle[1] - triangle[0]).cross(triangle[2] - triangle[0]).normalized();
        assert!(normal.z > 0.9, "a cap triangle faces {normal:?}, not the way the material went");
    }
}

#[test]
fn a_tube_is_capped_as_a_ring_so_its_wall_can_be_measured() {
    // The cut must show a wall, so the inner outline must come back as a hole.
    let outer = 20.0;
    let inner = 14.0;
    let tube = primitives::tube_mesh(outer, inner, 30.0, 64).weld();
    let plane = Plane::new(Vec3::new(0.0, 0.0, 1.0), 0.0);
    let outlines = loops(&tube, &plane);
    assert_eq!(outlines.len(), 2, "a tube's cut is an outer outline and a hole");
    let filled = cap(&tube, &plane);
    let expected = std::f64::consts::PI * ((outer / 2.0).powi(2) - (inner / 2.0).powi(2));
    // A 64-segment circle is slightly smaller than a true one: within a percent is tessellation.
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
    // Cutting a 20 mm box at z = 10 removes nothing, and a cap would fight the existing face.
    let plane = Plane::new(Vec3::new(0.0, 0.0, 1.0), 10.0);
    let mesh = box_mesh();
    assert!(loops(&mesh, &plane).is_empty(), "a cut that removed nothing produced an outline");
    for tri in &mesh.indices {
        let world = mesh.corners(*tri);
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

fn windowed(half: f64) -> Plane {
    Plane::new(Vec3::new(0.0, 0.0, 1.0), 0.0).within(Window {
        centre: Vec3::ZERO,
        u: Vec3::new(1.0, 0.0, 0.0),
        v: Vec3::new(0.0, 1.0, 0.0),
        half: [half, half],
    })
}

#[test]
fn a_window_takes_out_only_the_box_behind_it() {
    let plane = windowed(1.0);
    assert!(!plane.keeps(Vec3::new(0.0, 0.0, 5.0)), "straight behind the window stays");
    assert!(plane.keeps(Vec3::new(3.0, 0.0, 5.0)), "beside the window goes");
    assert!(plane.keeps(Vec3::new(0.0, 0.0, -5.0)), "in front of the plane goes");

    // A wall across the cut in x = 0 (z and y from -4 to 4); the window notches 2 x 4 out of its top half.
    let quad =
        [Vec3::new(0.0, -4.0, -4.0), Vec3::new(0.0, 4.0, -4.0), Vec3::new(0.0, 4.0, 4.0), Vec3::new(0.0, -4.0, 4.0)];
    let mut left = Vec::new();
    for tri in [[quad[0], quad[1], quad[2]], [quad[0], quad[2], quad[3]]] {
        let clipped = clip_triangle(&plane, tri);
        left.extend_from_slice(clipped.triangles());
    }
    assert!((area(&left) - (64.0 - 8.0)).abs() < 1e-9, "left {} of the wall", area(&left));
    assert!(left.iter().flatten().all(|&p| plane.keeps(p) || plane.walls().iter().any(|w| w.depth(p).abs() < 1e-9)));
}

#[test]
fn a_segment_through_a_window_loses_its_middle() {
    let plane = windowed(1.0);
    let pieces = kept_segments(&plane, Vec3::new(-5.0, 0.0, 2.0), Vec3::new(5.0, 0.0, 2.0));
    assert_eq!(pieces.len(), 2);
    assert!((pieces[0].1.x + 1.0).abs() < 1e-9 && (pieces[1].0.x - 1.0).abs() < 1e-9, "{:?}", &*pieces);
    // Beside the window, or in front of the plane, it is untouched.
    assert_eq!(kept_segments(&plane, Vec3::new(-5.0, 3.0, 2.0), Vec3::new(5.0, 3.0, 2.0)).len(), 1);
    assert_eq!(kept_segments(&plane, Vec3::new(-5.0, 0.0, -2.0), Vec3::new(5.0, 0.0, -2.0)).len(), 1);
}

#[test]
fn a_window_opens_five_faces_each_bounded_by_the_others() {
    let plane = windowed(1.0);
    let opened = faces(&plane);
    assert_eq!(opened.len(), 5);
    // The front face, cut down to the window.
    let big =
        [Vec3::new(-9.0, -9.0, 0.0), Vec3::new(9.0, -9.0, 0.0), Vec3::new(9.0, 9.0, 0.0), Vec3::new(-9.0, 9.0, 0.0)];
    let front = within(&big, &opened[0].bounds);
    assert!(front.iter().all(|p| p.x.abs() <= 1.0 + 1e-9 && p.y.abs() <= 1.0 + 1e-9), "{front:?}");
    // Only a triangle crossed inside the window touches it.
    let across = |x: f64| [Vec3::new(x, -0.5, -1.0), Vec3::new(x, 0.5, -1.0), Vec3::new(x, 0.0, 1.0)];
    assert!(triangle_touches(&plane, across(0.0)));
    assert!(!triangle_touches(&plane, across(3.0)));
}

#[test]
fn several_cuts_take_away_what_any_of_them_does() {
    // One plane removes the top, the other the right side; a 20-wide square in y = 0 keeps the quarter
    // neither reaches.
    let cuts = [Plane::new(Vec3::new(0.0, 0.0, 1.0), 0.0), Plane::new(Vec3::new(1.0, 0.0, 0.0), 0.0)];
    let square = [
        Vec3::new(-10.0, 0.0, -10.0),
        Vec3::new(10.0, 0.0, -10.0),
        Vec3::new(10.0, 0.0, 10.0),
        Vec3::new(-10.0, 0.0, 10.0),
    ];
    let mut left = Vec::new();
    for tri in [[square[0], square[1], square[2]], [square[0], square[2], square[3]]] {
        left.extend_from_slice(clip_by_all(&cuts, tri).triangles());
    }
    assert!((area(&left) - 100.0).abs() < 1e-9, "left {} of the square", area(&left));
    // A line across both loses what either removes.
    let kept = kept_by_all(&cuts, Vec3::new(-10.0, 0.0, -5.0), Vec3::new(10.0, 0.0, -5.0));
    assert_eq!(kept.len(), 1);
    assert!((kept[0].1.x).abs() < 1e-9, "{:?}", &*kept);
    assert!(kept_by_all(&cuts, Vec3::new(1.0, 0.0, 1.0), Vec3::new(5.0, 0.0, 5.0)).is_empty());
    // With no cut, a line is unchanged.
    assert_eq!(kept_by_all(&[], Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)).len(), 1);
}

#[test]
fn a_cut_knows_the_colour_of_the_body_it_opens() {
    // Issue 114: the cap is filled in the colour of the faces its outline was cut from.
    let red = crate::mesh::colour_tag([200, 30, 30]);
    let mut mesh = box_mesh();
    mesh.set_tag(red);
    let plane = Plane::new(Vec3::new(0.0, 0.0, 1.0), 0.0);
    let (outlines, tags) = tagged_loops(&mesh, &plane);
    let cap = fill(&outlines, plane.normal);
    assert!(!cap.is_empty(), "the box was not capped");
    assert!(cap.iter().all(|&t| tags.of_triangle(t) == red), "a cap triangle lost the box's colour");
}
