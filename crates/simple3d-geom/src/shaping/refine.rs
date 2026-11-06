//! Splitting a mesh's long edges until none is longer than asked.

use crate::mesh::{FastMap, Mesh};
use crate::vec3::Vec3;
use crate::Abandon;

/// `mesh` with every edge longer than `edge` split at its middle, repeated until none is, and
/// whether it stopped at `budget` triangles first. `None` when `give_up` says so.
///
/// An edge is split once for both of its triangles, so the surface stays closed; a triangle then
/// becomes two, three or four depending on how many of its edges were split. Midpoints lie on the
/// flat triangles, so the shape does not change, only how finely it is cut.
pub fn refine_until(mesh: &Mesh, edge: f64, budget: usize, give_up: Abandon<'_>) -> Option<(Mesh, bool)> {
    let mut mesh = mesh.weld();
    if edge.is_nan() || edge <= 0.0 {
        return Some((mesh, false));
    }
    let limit = edge * edge;
    loop {
        if give_up() {
            return None;
        }
        let mut mids: FastMap<(u32, u32), u32> = FastMap::default();
        let mut added = 0usize;
        let mut next_vertex = mesh.positions.len() as u32;
        for tri in &mesh.indices {
            let mut splits = 0;
            for k in 0..3 {
                let (a, b) = (tri[k], tri[(k + 1) % 3]);
                let (pa, pb) = (mesh.positions[a as usize], mesh.positions[b as usize]);
                let d = pb - pa;
                if d.dot(d) > limit {
                    splits += 1;
                    mids.entry(key(a, b)).or_insert_with(|| {
                        next_vertex += 1;
                        next_vertex - 1
                    });
                }
            }
            added += splits;
        }
        if mids.is_empty() {
            return Some((mesh, false));
        }
        // Each split edge adds one triangle on each side, so the count is known before building.
        if mesh.indices.len() + added > budget {
            return Some((mesh, true));
        }
        mesh = split(&mesh, &mids);
    }
}

fn key(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// One pass: every edge in `mids` split at its middle, numbered as given.
fn split(mesh: &Mesh, mids: &FastMap<(u32, u32), u32>) -> Mesh {
    let mut positions = mesh.positions.clone();
    positions.resize(mesh.positions.len() + mids.len(), Vec3::ZERO);
    for (&(a, b), &m) in mids {
        positions[m as usize] = (mesh.positions[a as usize] + mesh.positions[b as usize]) * 0.5;
    }
    let carry_sources = !mesh.sources.is_empty();
    let (mut indices, mut tags, mut sources) = (Vec::new(), Vec::new(), Vec::new());
    let p = |v: u32| positions[v as usize];
    for (index, tri) in mesh.indices.iter().enumerate() {
        let (tag, source) = (mesh.tag(index), mesh.source(index));
        let mut push = |t: [u32; 3]| {
            indices.push(t);
            tags.push(tag);
            if carry_sources {
                sources.push(source);
            }
        };
        let mid: [Option<u32>; 3] = std::array::from_fn(|k| mids.get(&key(tri[k], tri[(k + 1) % 3])).copied());
        match mid.iter().filter(|m| m.is_some()).count() {
            0 => push(*tri),
            1 => {
                // Turned so the split edge runs from corner 0 to 1.
                let r = mid.iter().position(Option::is_some).expect("one is split");
                let [a, b, c] = rotated(*tri, r);
                let m = mid[r].expect("split");
                push([a, m, c]);
                push([m, b, c]);
            }
            2 => {
                // Turned so the edge left whole runs from corner 2 back to 0.
                let whole = mid.iter().position(Option::is_none).expect("one is whole");
                let r = (whole + 1) % 3;
                let [a, b, c] = rotated(*tri, r);
                let (mab, mbc) = (mid[r].expect("split"), mid[(r + 1) % 3].expect("split"));
                push([mab, b, mbc]);
                // The quad a, mab, mbc, c, cut along its shorter diagonal.
                if (p(a) - p(mbc)).length() <= (p(mab) - p(c)).length() {
                    push([a, mab, mbc]);
                    push([a, mbc, c]);
                } else {
                    push([a, mab, c]);
                    push([mab, mbc, c]);
                }
            }
            _ => {
                let [a, b, c] = *tri;
                let [mab, mbc, mca] = mid.map(|m| m.expect("all split"));
                push([a, mab, mca]);
                push([mab, b, mbc]);
                push([mca, mbc, c]);
                push([mab, mbc, mca]);
            }
        }
    }
    Mesh { positions, indices, tags, sources }
}

/// `tri` turned so its corner `r` comes first, keeping the winding.
fn rotated(tri: [u32; 3], r: usize) -> [u32; 3] {
    [tri[r], tri[(r + 1) % 3], tri[(r + 2) % 3]]
}
