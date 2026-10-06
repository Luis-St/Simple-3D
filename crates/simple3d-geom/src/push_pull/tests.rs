use super::*;
use crate::primitives::box_mesh;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, BooleanOp};

fn volume(mesh: &crate::Mesh) -> f64 {
    mesh.signed_volume()
}

#[test]
fn a_square_with_a_hole_extrudes_into_a_closed_solid() {
    let square = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    let hole = vec![[3.0, 3.0], [3.0, 7.0], [7.0, 7.0], [7.0, 3.0]];
    let outline = Outline::new(square, vec![hole]).unwrap();
    assert!((outline.area() - 84.0).abs() < 1e-9);
    let mesh = extrude_outline(&outline, 2.0);
    assert!(mesh.manifold_issue().is_none(), "{:?}", mesh.manifold_issue());
    assert!((volume(&mesh) - 168.0).abs() < 1e-6, "volume {}", volume(&mesh));
}

#[test]
fn a_clockwise_outline_is_turned_round_and_collinear_points_dropped() {
    let outline = Outline::new(vec![[0.0, 0.0], [0.0, 4.0], [4.0, 4.0], [4.0, 2.0], [4.0, 0.0]], vec![]).unwrap();
    assert_eq!(outline.outer.len(), 4, "the point on the edge stayed: {:?}", outline.outer);
    assert!(outline.area() > 0.0);
}

#[test]
fn the_top_of_a_box_is_one_flat_face_with_one_loop() {
    let mesh = box_mesh(10.0, 20.0, 30.0);
    let top = (0..mesh.indices.len()).find(|&t| mesh.triangle_normal(mesh.indices[t]).z > 0.9).unwrap();
    let face = flat_face(&mesh, top).unwrap();
    assert_eq!(face.triangles.len(), 2);
    assert_eq!(face.loops.len(), 1);
    assert!(!face.curved);
    assert!((face.centre - Vec3::new(0.0, 0.0, 15.0)).length() < 1e-9);
    let (u, v) = face_basis(face.normal);
    let outline = face.outline(face.centre, u, v).unwrap();
    assert!((outline.area() - 200.0).abs() < 1e-6);
}

#[test]
fn a_drilled_plate_top_keeps_the_hole_and_a_cylinder_side_is_curved() {
    let plate = box_mesh(40.0, 40.0, 4.0);
    let drill = crate::primitives::cylinder_mesh(10.0, 10.0, 10.0, 32);
    let drilled = evaluate_boolean(BooleanOp::Difference, &[plate, drill.clone()]);
    let top = (0..drilled.indices.len()).find(|&t| drilled.triangle_normal(drilled.indices[t]).z > 0.9).unwrap();
    let face = flat_face(&drilled, top).unwrap();
    assert_eq!(face.loops.len(), 2, "the hole is not a loop of its own");
    let (u, v) = face_basis(face.normal);
    let outline = face.outline(face.centre, u, v).unwrap();
    assert_eq!(outline.holes.len(), 1);
    assert!(outline.area() < 1600.0 && outline.area() > 1500.0, "area {}", outline.area());

    let side = (0..drill.indices.len()).find(|&t| drill.triangle_normal(drill.indices[t]).z.abs() < 0.1).unwrap();
    assert!(flat_face(&drill, side).unwrap().curved, "a cylinder's facet was taken for a face");
}

#[test]
fn sources_survive_a_union_and_a_difference() {
    let mut a = box_mesh(10.0, 10.0, 10.0);
    a.set_source(7);
    let mut b = box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(5.0, 0.0, 5.0));
    b.set_source(9);
    for op in [BooleanOp::Union, BooleanOp::Difference] {
        let result = evaluate_boolean(op, &[a.clone(), b.clone()]);
        assert_eq!(result.sources.len(), result.indices.len());
        for t in 0..result.indices.len() {
            let source = result.source(t);
            assert!(source == 7 || source == 9, "{op:?} lost a source: {source}");
        }
        // The top of `a` left of `b` is still `a`'s.
        let left_top = (0..result.indices.len()).find(|&t| {
            let [p, q, r] = result.corners(result.indices[t]);
            let c = (p + q + r) / 3.0;
            result.triangle_normal(result.indices[t]).z > 0.9 && c.x < -1.0 && (c.z - 5.0).abs() < 1e-6
        });
        assert_eq!(left_top.map(|t| result.source(t)), Some(7), "{op:?}");
    }
}

#[test]
fn a_flush_face_of_another_object_is_not_part_of_the_face() {
    // Two boxes stacked, their front faces in one plane and joined by the union.
    let mut lower = box_mesh(20.0, 20.0, 20.0);
    lower.set_source(1);
    let mut upper = box_mesh(20.0, 20.0, 20.0).translated(Vec3::new(0.0, 0.0, 20.0));
    upper.set_source(2);
    let both = evaluate_boolean(BooleanOp::Union, &[lower, upper]);
    let front = (0..both.indices.len())
        .find(|&t| {
            let [a, b, c] = both.corners(both.indices[t]);
            both.triangle_normal(both.indices[t]).y < -0.9 && (a.z + b.z + c.z) / 3.0 > 20.0
        })
        .unwrap();
    let face = flat_face(&both, front).unwrap();
    assert!(face.triangles.iter().all(|&t| both.source(t) == 2), "the face ran onto the lower box");
    assert!(!face.curved, "the flush neighbour was taken for a curve");
    let (u, v) = face_basis(face.normal);
    assert!((face.outline(face.centre, u, v).unwrap().area() - 400.0).abs() < 1e-6);
}
