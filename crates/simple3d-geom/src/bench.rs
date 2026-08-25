#![cfg(test)]
//! Timings and soundness checks for the boolean kernel, so a change to it is
//! measured rather than argued about.
//!
//! `#[ignore]`d: the larger segment counts take minutes and there is nothing
//! here to assert in the ordinary suite. Run with
//!
//! ```text
//! cargo test --release -p simple3d-geom -- --ignored --nocapture bench
//! ```
//!
//! and always in release -- the kernel is float-heavy, and a debug build
//! measures the absence of optimisation rather than the algorithm.

use crate::mesh::Mesh;
use crate::{primitives, BooleanOp};
use std::time::Instant;

fn union_of(a: &Mesh, b: &Mesh) -> Mesh {
    crate::evaluate_boolean(BooleanOp::Union, &[a.clone(), b.clone()])
}

/// The case `KNOWN_ISSUES.md` measures: a convex round operand meeting a plate.
/// Both the cost and whether the result is still a closed solid.
#[test]
#[ignore]
fn bench_cap_union_plate() {
    println!("{:>9}  {:>10}  {:>10}  {:>9}  {}", "segments", "in tris", "time", "out tris", "manifold");
    for segments in [32, 64, 128, 224, 256, 320] {
        let cap = primitives::spherical_cap_mesh(20.0, 6.0, segments);
        let plate = primitives::plate_mesh(40.0, 40.0, 4.0);
        let input = cap.triangle_count() + plate.triangle_count();
        let at = Instant::now();
        let result = union_of(&cap, &plate);
        let elapsed = at.elapsed();
        let issue = result.manifold_issue();
        println!(
            "{segments:>9}  {input:>10}  {:>10.3?}  {:>9}  {}",
            elapsed,
            result.triangle_count(),
            match &issue {
                None => "yes".to_string(),
                Some(why) => format!("NO -- {why}"),
            }
        );
    }
}

/// Two dense convex operands actually crossing: the case where both BSP trees
/// are chains.
#[test]
#[ignore]
fn bench_two_round_operands() {
    for segments in [32, 64, 128, 256] {
        let a = primitives::cylinder_mesh(20.0, 20.0, 40.0, segments);
        let b = primitives::cylinder_mesh(20.0, 20.0, 40.0, segments)
            .transformed(crate::Vec3::new(6.0, 0.0, 0.0), crate::Vec3::new(0.0, 90.0, 0.0));
        let at = Instant::now();
        let result = union_of(&a, &b);
        println!(
            "cylinder({segments}) union cylinder: {:>10.3?}  {} tris  manifold {}",
            at.elapsed(),
            result.triangle_count(),
            result.manifold_issue().is_none()
        );
    }
}

/// Nothing overlaps: this must stay linear, and says whether the disjoint fast
/// path is holding.
#[test]
#[ignore]
fn bench_many_disjoint() {
    for count in [16usize, 64, 256] {
        let meshes: Vec<Mesh> = (0..count)
            .map(|i| {
                let (x, y) = ((i % 16) as f64 * 30.0, (i / 16) as f64 * 30.0);
                primitives::box_mesh(20.0, 20.0, 20.0).transformed(crate::Vec3::new(x, y, 0.0), crate::Vec3::ZERO)
            })
            .collect();
        let at = Instant::now();
        let result = crate::evaluate_boolean(BooleanOp::Union, &meshes);
        println!("{count:>4} disjoint boxes: {:>10.3?}  {} tris", at.elapsed(), result.triangle_count());
    }
}
