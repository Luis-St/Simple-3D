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

/// A boundary pinched in the middle is cut into two loops, and nothing on it
/// is lost. This is the shape the pass used to mishandle: a vertex spliced
/// into two of a triangle's edges at once, which is what happens around the
/// needle triangles a BSP produces.
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

/// The pinch that actually turns up: the repeated vertex sits a hair from a
/// corner, on both of the edges meeting there. The piece it cuts off is that
/// corner and nothing else -- two vertices, enclosing no area -- so it is
/// dropped rather than emitted as a degenerate triangle, and the corner goes
/// with it. That is the right answer geometrically: the vertex and the corner
/// are within tolerance of each other's edges, so the wedge between them has
/// no surface to contribute, and the neighbouring triangles still carry the
/// corner. `a_dense_round_operand_meeting_a_plate_stays_manifold` is what
/// checks that no hole is left behind by it.
#[test]
fn a_pinch_against_a_corner_drops_the_wedge_that_has_no_area() {
    let boundary = [0, 9, 1, 4, 5, 6, 7, 8, 9];
    let loops = split_pinched_loops(&boundary);
    assert_eq!(loops, vec![vec![9, 1, 4, 5, 6, 7, 8]]);
    assert!(is_simple(&loops[0]));
}

/// An ordinary boundary -- every vertex distinct -- is one loop, unchanged.
#[test]
fn an_unpinched_boundary_is_left_as_one_loop() {
    let boundary = [0, 1, 2, 3, 4];
    assert_eq!(split_pinched_loops(&boundary), vec![vec![0, 1, 2, 3, 4]]);
}

/// A union of two finely tessellated operands is a closed solid. At 224 and
/// 256 segments this came out with holes and doubled edges before the pinch
/// was handled; 144 is the smallest form of the same case that still runs in
/// about a second.
#[test]
fn a_dense_round_operand_meeting_a_plate_stays_manifold() {
    let cap = crate::primitives::spherical_cap_mesh(20.0, 6.0, 144);
    let plate = crate::primitives::plate_mesh(40.0, 40.0, 4.0);
    let result = crate::evaluate_boolean(crate::BooleanOp::Union, &[cap, plate]);
    assert_eq!(result.manifold_issue(), None, "the union is not a closed solid");
}
/// A closed body with a small tetrahedral pocket beside it that is missing
/// one of its faces: a three-vertex hole, and the lid that fills it is the
/// face that was taken out.
#[test]
fn a_hole_small_enough_to_be_a_defect_is_given_its_lid() {
    let open = open_tetrahedron(0.1);
    assert!(open.manifold_issue().is_some(), "this test needs an open surface to start with");
    let capped = cap_boundary_loops(open.clone());
    assert_eq!(capped.manifold_issue(), None, "the hole was not closed");
    assert_eq!(capped.triangle_count(), open.triangle_count() + 1);
}

/// The same hole, at a size no lid is safe at. A boundary loop a tenth of
/// the model across is a wrong answer rather than a missing triangle, and
/// covering it over would hide that where reporting it does not.
#[test]
fn a_hole_the_size_of_the_model_is_left_open() {
    let open = open_tetrahedron(20.0);
    assert!(cap_boundary_loops(open).manifold_issue().is_some(), "a hole this size must not be filled");
}

/// A 100 mm closed box, and beside it a tetrahedron of the given size with
/// its base missing. The box is there to be the model: what may be capped
/// is judged against the size of what is being repaired, so a hole has to
/// be small relative to *something*.
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
