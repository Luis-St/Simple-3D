use crate::mesh::Mesh;
use crate::vec3::Vec3;

use super::*;

fn is_simple(piece: &[u32]) -> bool {
    let mut seen = piece.to_vec();
    seen.sort_unstable();
    let before = seen.len();
    seen.dedup();
    seen.len() == before
}

/// A boundary pinched in the middle becomes two loops with nothing lost: a vertex spliced into two
/// of a triangle's edges, as around BSP needles.
#[test]
fn a_pinched_boundary_is_cut_into_two_simple_loops() {
    let boundary = [0, 1, 2, 3, 4, 2, 5, 6];
    let loops = split_pinched_loops(&boundary);
    assert_eq!(loops.len(), 2, "got {loops:?}");
    for piece in &loops {
        assert!(is_simple(piece), "piece {piece:?} still visits a vertex twice");
        assert!(piece.len() >= 3, "piece {piece:?} encloses no area");
    }
    let mut covered: Vec<u32> = loops.iter().flatten().copied().collect();
    covered.sort_unstable();
    covered.dedup();
    assert_eq!(covered, vec![0, 1, 2, 3, 4, 5, 6], "a vertex of the boundary was lost");
}

/// The realistic pinch: the repeated vertex a hair from a corner, on both edges there. The piece
/// cut off encloses no area and is dropped; the neighbours still carry the corner
/// (`a_dense_round_operand_meeting_a_plate_stays_manifold` checks no hole is left).
#[test]
fn a_pinch_against_a_corner_drops_the_wedge_that_has_no_area() {
    let boundary = [0, 9, 1, 4, 5, 6, 7, 8, 9];
    let loops = split_pinched_loops(&boundary);
    assert_eq!(loops, vec![vec![9, 1, 4, 5, 6, 7, 8]]);
    assert!(is_simple(&loops[0]));
}

/// An ordinary boundary with distinct vertices is one loop, unchanged.
#[test]
fn an_unpinched_boundary_is_left_as_one_loop() {
    let boundary = [0, 1, 2, 3, 4];
    assert_eq!(split_pinched_loops(&boundary), vec![vec![0, 1, 2, 3, 4]]);
}

/// A union of two finely tessellated operands is closed; at 224 and 256 segments it had holes
/// before pinches were handled. 144 is the smallest quick form of the case.
#[test]
fn a_dense_round_operand_meeting_a_plate_stays_manifold() {
    let cap = crate::primitives::spherical_cap_mesh(20.0, 6.0, 144);
    let plate = crate::primitives::plate_mesh(40.0, 40.0, 4.0);
    let result = crate::evaluate_boolean(crate::BooleanOp::Union, &[cap, plate]);
    assert_eq!(result.manifold_issue(), None, "the union is not a closed solid");
}
/// A closed body plus a tetrahedral pocket missing one face: the lid is the missing face.
#[test]
fn a_hole_small_enough_to_be_a_defect_is_given_its_lid() {
    let open = open_tetrahedron(0.1);
    assert!(open.manifold_issue().is_some(), "this test needs an open surface to start with");
    let capped = cap_boundary_loops(open.clone());
    assert_eq!(capped.manifold_issue(), None, "the hole was not closed");
    assert_eq!(capped.triangle_count(), open.triangle_count() + 1);
}

/// The same hole at a size no lid is safe at: a tenth of the model is a wrong answer, reported
/// rather than covered.
#[test]
fn a_hole_the_size_of_the_model_is_left_open() {
    let open = open_tetrahedron(20.0);
    assert!(cap_boundary_loops(open).manifold_issue().is_some(), "a hole this size must not be filled");
}

/// A 100 mm closed box plus an open-based tetrahedron of `size`; the box sets the scale holes are
/// judged against.
fn open_tetrahedron(size: f64) -> Mesh {
    let mut mesh = crate::primitives::box_mesh(100.0, 100.0, 100.0);
    let at = Vec3::new(80.0, 0.0, 0.0);
    let p = [at, at + Vec3::new(size, 0.0, 0.0), at + Vec3::new(0.0, size, 0.0), at + Vec3::new(0.0, 0.0, size)];
    // Three of the four faces, wound outwards; the base (0, 2, 1) is left out.
    mesh.push_triangle(p[0], p[1], p[3]);
    mesh.push_triangle(p[1], p[2], p[3]);
    mesh.push_triangle(p[2], p[0], p[3]);
    weld_tolerant(&mesh, WELD_TOL)
}

/// A lid over a collinear slit has no area, which the exporter refuses; splitting its neighbour at
/// the middle corner removes the needle and keeps the solid closed.
#[test]
fn a_needle_lid_is_traded_for_a_split_of_its_neighbour() {
    let v = |x, y, z| Vec3::new(x, y, z);
    // A tetrahedron whose front face is split at its bottom edge's middle, the slit closed by a lid.
    let mesh = Mesh {
        positions: vec![v(0.0, 0.0, 0.0), v(0.5, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0), v(0.0, 0.0, 1.0)],
        indices: vec![[0, 3, 2], [0, 1, 4], [1, 2, 4], [0, 4, 3], [2, 3, 4], [0, 2, 1]],
        tags: Vec::new(),
    };
    let area_free = |m: &Mesh| m.indices.iter().filter(|&&t| m.triangle_normal(t).length() < 0.5).count();
    assert_eq!(mesh.manifold_issue(), None, "this test needs a closed surface to start with");
    assert_eq!(area_free(&mesh), 1, "this test needs a needle to start with");

    let split = split_needles(mesh.clone(), WELD_TOL);
    assert_eq!(split.manifold_issue(), None, "the split opened the surface");
    assert_eq!(area_free(&split), 0, "a triangle with no area is left");
    assert_eq!(split.triangle_count(), mesh.triangle_count());
}
