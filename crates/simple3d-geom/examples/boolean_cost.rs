//! What one assembly costs, step by step.
//!
//! `cargo run --release -p simple3d-geom --example boolean_cost`
//!
//! The fixture is one of the fifty assemblies in the spec's 200-primitive scene
//! (`simple3d-core/tests/performance.rs`): a plate with a hole and a slot cut
//! from it, and a boss unioned on. Both numbers matter and they pull against
//! each other -- the triangle count of each step is the input size of the next,
//! so a boolean that leaves the result denser than it needs to be makes every
//! later boolean in the chain slower.
//!
//! Reference figures on the machine this was written on, for spotting a
//! regression: ~4 ms and 228 triangles for the whole assembly.

use simple3d_geom::{csg_bsp, primitives, Mesh, Vec3};
use std::time::Instant;

fn step(label: &str, f: impl FnOnce() -> Mesh) -> Mesh {
    let started = Instant::now();
    let mesh = f();
    println!("{label:<16} {:>9.3?}  {:>5} triangles", started.elapsed(), mesh.triangle_count());
    mesh
}

fn main() {
    let plate = primitives::box_mesh(40.0, 20.0, 4.0);
    let hole = primitives::cylinder_mesh(6.0, 6.0, 20.0, 16).translated(Vec3::new(-12.0, 0.0, 0.0));
    let slot = primitives::box_mesh(8.0, 5.0, 20.0).translated(Vec3::new(12.0, 0.0, 0.0));
    let boss = primitives::cylinder_mesh(9.0, 9.0, 5.0, 16);

    let started = Instant::now();
    let drilled = step("subtract hole", || csg_bsp::subtract(&plate, &hole));
    let slotted = step("subtract slot", || csg_bsp::subtract(&drilled, &slot));
    let assembly = step("union boss", || csg_bsp::union(&slotted, &boss));
    println!("{:<16} {:>9.3?}", "assembly", started.elapsed());

    let (lo, hi) = assembly.bounds().unwrap();
    println!("\nbounding box     {:?}", hi - lo);
    match assembly.manifold_issue() {
        Some(issue) => println!("NOT MANIFOLD:    {issue}"),
        None => println!("manifold         yes"),
    }
}
