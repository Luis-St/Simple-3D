//! What a difference of many cutters costs, folded one cutter at a time
//! against subtracting their union once.
//!
//! `cargo run --release -p simple3d-geom --example difference_cost`
//!
//! Reference figures on the machine this was written on: 3.4 s folded against
//! 0.27 s evaluated for the 200-segment sphere with twenty cutters.

use simple3d_geom::{csg_bsp, evaluate_boolean, primitives, BooleanOp, Mesh, Vec3};
use std::time::Instant;

fn main() {
    for (segments, cutters) in [(64, 8), (128, 20), (200, 20), (96, 60)] {
        let base = primitives::ellipsoid_mesh(40.0, 40.0, 40.0, segments);
        let mut children = vec![base.clone()];
        for i in 0..cutters {
            let angle = i as f64 / cutters as f64 * std::f64::consts::TAU;
            let at = Vec3::new(angle.cos() * 19.0, angle.sin() * 19.0, (i % 3) as f64 * 6.0 - 6.0);
            children.push(primitives::cylinder_mesh(4.0, 4.0, 12.0, 32).translated(at));
        }
        let started = Instant::now();
        let folded = children[1..].iter().fold(base.clone(), |acc, cutter| csg_bsp::subtract(&acc, cutter));
        let fold_time = started.elapsed();
        let started = Instant::now();
        let once: Mesh = evaluate_boolean(BooleanOp::Difference, &children);
        let once_time = started.elapsed();
        let started = Instant::now();
        let cutters_union = evaluate_boolean(BooleanOp::Union, &children[1..]);
        let union_time = started.elapsed();
        let single = csg_bsp::subtract(&base, &cutters_union);
        let single_time = started.elapsed();
        println!(
            "    one subtract of the union: {:>9.3?} (union {:>9.3?}) ({} tris, manifold {})",
            single_time,
            union_time,
            single.triangle_count(),
            single.manifold_issue().is_none()
        );
        println!(
            "{:>3} segments, {:>2} cutters: folded {:>9.3?} ({} tris, manifold {}), evaluate {:>9.3?} ({} tris, manifold {})",
            segments,
            cutters,
            fold_time,
            folded.triangle_count(),
            folded.manifold_issue().is_none(),
            once_time,
            once.triangle_count(),
            once.manifold_issue().is_none(),
        );
    }
}
