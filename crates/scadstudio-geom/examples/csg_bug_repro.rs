//! Minimal, reliably-reproducing repro for a known bug in the BSP CSG
//! kernel (see `KNOWN_ISSUES.md` at the repo root). `cargo test -p
//! scadstudio-geom` already fails on `boolean_diff_simple_boxes` and
//! `boolean_difference_hole_in_plate_is_manifold`; this prints the actual
//! bad edges for the simplest of the two so the failure is visible without
//! re-deriving the debug harness from scratch.
//!
//! Run with: cargo run -p scadstudio-geom --example csg_bug_repro

use scadstudio_geom::primitives;

fn main() {
    let a = primitives::box_mesh(20.0, 20.0, 20.0);
    let b = primitives::box_mesh(10.0, 10.0, 30.0); // pokes through a's top and bottom faces
    let result = scadstudio_geom::csg_bsp::subtract(&a, &b);
    let welded = result.weld();
    println!("subtract(20x20x20 box, 10x10x30 box): {} triangles after welding", welded.indices.len());

    use std::collections::HashMap;
    let mut directed: HashMap<(u32, u32), i32> = HashMap::new();
    for tri in &welded.indices {
        for i in 0..3 {
            *directed.entry((tri[i], tri[(i + 1) % 3])).or_insert(0) += 1;
        }
    }
    let mut bad: Vec<_> = directed
        .iter()
        .filter(|(&(a, b), &c)| c != 1 || !directed.contains_key(&(b, a)))
        .map(|(&(a, b), _)| (a, b))
        .collect();
    bad.sort();

    if bad.is_empty() {
        println!("No bad edges found -- looks like the bug is fixed!");
        return;
    }
    println!("{} bad (unpaired) directed edges, e.g.:", bad.len());
    for (a, b) in bad.iter().take(8) {
        println!("  ({a},{b})  pos_a={:?}  pos_b={:?}", welded.positions[*a as usize], welded.positions[*b as usize]);
    }
    println!();
    println!("See KNOWN_ISSUES.md for what's been ruled out and where to look next.");
}
