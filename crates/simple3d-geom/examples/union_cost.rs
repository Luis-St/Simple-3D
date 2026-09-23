//! What a union of one large body with many small ones costs.
//!
//! `cargo run --release -p simple3d-geom --example union_cost`
//!
//! Reference figures on the machine this was written on: 0.30 s for the
//! 200-segment sphere with twenty bosses, which took 3.4 s merged one at a time.

use simple3d_geom::{evaluate_boolean_traced, primitives, BooleanOp, Vec3};
use std::time::Instant;

fn main() {
    for (segments, bosses, spacing) in [(64, 8, 19.0), (128, 20, 19.0), (200, 20, 19.0), (96, 60, 19.0)] {
        let base = primitives::ellipsoid_mesh(40.0, 40.0, 40.0, segments);
        let mut children = vec![base];
        for i in 0..bosses {
            let angle = i as f64 / bosses as f64 * std::f64::consts::TAU;
            let at = Vec3::new(angle.cos() * spacing, angle.sin() * spacing, (i % 3) as f64 * 6.0 - 6.0);
            children.push(primitives::cylinder_mesh(4.0, 4.0, 12.0, 32).translated(at));
        }
        // And some that meet nothing, which must come through untouched.
        for i in 0..5 {
            children.push(primitives::box_mesh(2.0, 2.0, 2.0).translated(Vec3::new(100.0 + i as f64 * 5.0, 0.0, 0.0)));
        }
        let started = Instant::now();
        let (mesh, untouched) = evaluate_boolean_traced(BooleanOp::Union, &children, &simple3d_geom::never);
        println!(
            "{:>3} segments, {:>2} bosses: {:>9.3?} ({} tris, manifold {}, {} untouched)",
            segments,
            bosses,
            started.elapsed(),
            mesh.triangle_count(),
            mesh.manifold_issue().is_none(),
            untouched.len()
        );
    }
}
