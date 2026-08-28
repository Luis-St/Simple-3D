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
///
/// 384 is in the list because it was the one segment count in it that came back
/// non-manifold, and 448 because it was the one that took two minutes.
#[test]
#[ignore]
fn bench_cap_union_plate() {
    println!("{:>9}  {:>10}  {:>10}  {:>9}  {}", "segments", "in tris", "time", "out tris", "manifold");
    for segments in [32, 64, 128, 224, 256, 320, 384, 448] {
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
fn bench_boolean_audit() {
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

/// Every hole the audit leaves, as boundary loops with their size, so a repair
/// is chosen against what is actually there.
#[test]
#[ignore]
fn bench_hole_sizes() {
    use crate::Vec3;
    use std::collections::BTreeMap;
    let segments = 128u32;
    let shapes: [(&str, Mesh); 5] = [
        ("sphere", primitives::ellipsoid_mesh(20.0, 20.0, 20.0, segments)),
        ("cap", primitives::spherical_cap_mesh(20.0, 6.0, segments)),
        ("cyl", primitives::cylinder_mesh(20.0, 20.0, 30.0, segments)),
        ("torus", primitives::torus_mesh(30.0, 8.0, 360.0, 96)),
        ("plate", primitives::plate_mesh(40.0, 40.0, 4.0)),
    ];
    for (na, a) in &shapes {
        for (nb, b) in &shapes {
            if na == nb {
                continue;
            }
            let b = b.transformed(Vec3::new(3.0, 2.0, 1.0), Vec3::new(10.0, 0.0, 0.0));
            for op in [BooleanOp::Union, BooleanOp::Difference, BooleanOp::Intersection] {
                let r = crate::evaluate_boolean(op, &[a.clone(), b.clone()]);
                if r.manifold_issue().is_none() {
                    continue;
                }
                let mut count: BTreeMap<(u32, u32), i32> = BTreeMap::new();
                for t in &r.indices {
                    for k in 0..3 {
                        *count.entry((t[k], t[(k + 1) % 3])).or_insert(0) += 1;
                    }
                }
                let mut next: BTreeMap<u32, u32> = BTreeMap::new();
                let mut extra = 0;
                for (&(x, y), &c) in &count {
                    let rev = count.get(&(y, x)).copied().unwrap_or(0);
                    if c > rev {
                        if next.insert(x, y).is_some() {
                            extra += 1;
                        }
                    }
                }
                print!("{na} {op:?} {nb}: {} boundary edges", next.len());
                if extra > 0 {
                    print!(" ({extra} vertices with more than one)");
                }
                // Chain them into loops.
                let mut seen: std::collections::BTreeSet<u32> = Default::default();
                let mut loops: Vec<(usize, f64)> = Vec::new();
                let mut dangling = 0;
                for &start in next.keys() {
                    if seen.contains(&start) {
                        continue;
                    }
                    let (mut cur, mut n) = (start, 0usize);
                    let (mut lo, mut hi) = (Vec3::splat(f64::MAX), Vec3::splat(f64::MIN));
                    loop {
                        if !seen.insert(cur) {
                            break;
                        }
                        let p = r.positions[cur as usize];
                        lo = lo.min(p);
                        hi = hi.max(p);
                        n += 1;
                        match next.get(&cur) {
                            Some(&nx) => cur = nx,
                            None => {
                                dangling += 1;
                                break;
                            }
                        }
                        if cur == start {
                            loops.push((n, (hi - lo).length()));
                            break;
                        }
                    }
                }
                loops.sort_by(|a, b| b.1.total_cmp(&a.1));
                println!(
                    "  -> {} closed loops {:?}{}",
                    loops.len(),
                    loops.iter().take(6).map(|&(n, d)| format!("{n}v/{d:.6}mm")).collect::<Vec<_>>(),
                    if dangling > 0 { format!(", {dangling} open chains") } else { String::new() }
                );
            }
        }
    }
}
