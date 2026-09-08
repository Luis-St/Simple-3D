//! Rebuilding a flat region from its boundary.

use super::*;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, primitives, BooleanOp};

#[test]
pub(crate) fn rebuilding_a_flat_region_keeps_its_boundary() {
    // The one thing retriangulation must never do is straighten a face's
    // boundary and leave the neighbouring face still bent to the old shape --
    // that is a T-junction, and slicers reject it. Nine holes in a row give the
    // top face a boundary made almost entirely of collinear split points, which
    // is exactly the case that stresses it.
    let mut operands = vec![primitives::box_mesh(100.0, 20.0, 4.0)];
    for i in 0..9 {
        operands.push(primitives::cylinder_mesh(6.0, 6.0, 20.0, 12).translated(Vec3::new(
            -40.0 + i as f64 * 10.0,
            0.0,
            0.0,
        )));
    }
    let result = evaluate_boolean(BooleanOp::Difference, &operands);
    assert_manifold("nine holes", &result);
    assert_bounds("nine holes", &result, Vec3::new(100.0, 20.0, 4.0), 1e-9);

    // Every hole is still open: no vertex may sit inside one.
    for i in 0..9 {
        let centre_x = -40.0 + i as f64 * 10.0;
        for p in &result.positions {
            let r = ((p.x - centre_x).powi(2) + p.y * p.y).sqrt();
            assert!(r > 3.0 * (std::f64::consts::PI / 12.0).cos() - 1e-6, "a vertex landed inside hole {i}");
        }
    }
}

#[test]
pub(crate) fn a_region_that_cannot_be_rebuilt_keeps_its_original_triangles() {
    // Retriangulation is allowed to give up, and when it does the region must
    // come through untouched rather than half-rebuilt. Two coplanar squares
    // meeting at one corner are the smallest case it must refuse: the boundary
    // leaves that corner two ways, and which one continues the loop is a guess.
    let mut region = Mesh::new();
    let quad = |m: &mut Mesh, x: f64, y: f64| {
        let p = |dx: f64, dy: f64| Vec3::new(x + dx, y + dy, 0.0);
        m.push_triangle(p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0));
        m.push_triangle(p(0.0, 0.0), p(10.0, 10.0), p(0.0, 10.0));
    };
    quad(&mut region, 0.0, 0.0);
    quad(&mut region, 10.0, 10.0);
    // Welded, so the shared corner really is one vertex and the pinch is real.
    let region = region.weld();

    let rebuilt = crate::planar::retriangulate_flat_regions(&region);
    assert_eq!(rebuilt.triangle_count(), region.triangle_count(), "a pinched region was rebuilt anyway");
    let (lo, hi) = rebuilt.bounds().unwrap();
    assert_eq!((lo, hi), region.bounds().unwrap());
}
