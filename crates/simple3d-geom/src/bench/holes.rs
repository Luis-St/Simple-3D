//! How the cost of subtracting a hole moves with the size of it.

use super::*;
use crate::{primitives, BooleanOp};

/// Every hole the audit leaves, as boundary loops with their size, so a repair
/// is chosen against what is actually there.
#[test]
#[ignore]
pub(crate) fn bench_hole_sizes() {
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
