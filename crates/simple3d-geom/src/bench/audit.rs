//! Whether the kernel's answers are still sound across the shapes that
//! have broken it before.

use super::*;
use crate::{primitives, BooleanOp};

/// Every ordered pair of primitives, through every operation, checked for a
/// closed surface and for a volume the operation allows.
///
/// The suite's own boolean tests are all on shapes meeting squarely. This meets
/// them at an angle and off-centre, which is where the kernel's fixed-epsilon
/// classification actually fails, and it is how the counts in `KNOWN_ISSUES.md`
/// were arrived at: 22 of these pairs were not closed before the near-face
/// clip, 18 after, 9 once the general path clipped against near faces too, and
/// none once `repair` learned to put a lid on the hole that is left. It asserts
/// that now. Still `#[ignore]`d only because it takes a quarter of a minute.
#[test]
#[ignore]
pub(crate) fn bench_boolean_audit() {
    use crate::Vec3;
    let segments = 128u32;
    let shapes: [(&str, Mesh); 5] = [
        ("sphere", primitives::ellipsoid_mesh(20.0, 20.0, 20.0, segments)),
        ("cap", primitives::spherical_cap_mesh(20.0, 6.0, segments)),
        ("cyl", primitives::cylinder_mesh(20.0, 20.0, 30.0, segments)),
        ("torus", primitives::torus_mesh(30.0, 8.0, 360.0, 96)),
        ("plate", primitives::plate_mesh(40.0, 40.0, 4.0)),
    ];
    let volume = |m: &Mesh| {
        let mut v = 0.0;
        for t in &m.indices {
            let (a, b, c) = (m.positions[t[0] as usize], m.positions[t[1] as usize], m.positions[t[2] as usize]);
            v += a.dot(b.cross(c)) / 6.0;
        }
        v
    };
    let (mut pairs, mut open) = (0, 0);
    for (na, a) in &shapes {
        for (nb, b) in &shapes {
            if na == nb {
                continue;
            }
            // Off-centre and off-axis on purpose: squarely meeting shapes hide
            // the grazing contacts that break plane classification.
            let b = b.transformed(Vec3::new(3.0, 2.0, 1.0), Vec3::new(10.0, 0.0, 0.0));
            for op in [BooleanOp::Union, BooleanOp::Difference, BooleanOp::Intersection] {
                let result = crate::evaluate_boolean(op, &[a.clone(), b.clone()]);
                if result.triangle_count() == 0 {
                    continue;
                }
                pairs += 1;
                let (va, vb, vr) = (volume(a), volume(&b), volume(&result));
                let (lo, hi) = match op {
                    BooleanOp::Union => (va.max(vb), va + vb),
                    BooleanOp::Difference => (0.0, va),
                    BooleanOp::Intersection => (0.0, va.min(vb)),
                    BooleanOp::Hull => (0.0, f64::MAX),
                };
                if vr < lo - 1e-6 || vr > hi + 1e-6 {
                    println!("{na} {op:?} {nb}: volume {vr:.4} outside [{lo:.4}, {hi:.4}]");
                }
                if let Some(why) = result.manifold_issue() {
                    open += 1;
                    println!("{na} {op:?} {nb}: not closed -- {why}");
                }
            }
        }
    }
    println!("{open} of {pairs} pairs are not closed");
    assert_eq!(open, 0, "every pair must come out a closed solid");
}
