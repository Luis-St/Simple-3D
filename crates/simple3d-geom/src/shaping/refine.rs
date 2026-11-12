//! Splitting a mesh's long edges until none is longer than asked.

use crate::mesh::{FastMap, Mesh};
use crate::topology::{corner_normals, Topology, SMOOTH_CREASE};
use crate::vec3::Vec3;
use crate::Abandon;

/// `mesh` with every edge longer than `edge` split at its middle, repeated until none is, and
/// whether it stopped at `budget` triangles first. `None` when `give_up` says so.
///
/// An edge is split once for both of its triangles, so the surface stays closed; a triangle then
/// becomes two, three or four depending on how many of its edges were split. Each new vertex is
/// curved out onto the smooth surface the mesh is shaded as ([`curved`]): left on the flat
/// triangles, a 32-segment cylinder's facets turned into flat patches, which its smooth shading then
/// showed as bands. Flat faces and corners sharper than [`SMOOTH_CREASE`] keep their shape.
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
        let bulges = curved(&mesh, &mids);
        mesh = split(&mesh, &mids);
        for (m, bulge) in bulges {
            mesh.positions[m as usize] = mesh.positions[m as usize] + bulge;
        }
    }
}

/// How far each new vertex in `mids` moves off its edge's middle onto the smooth surface through the
/// edge's ends: half way to their tangent planes (Phong tessellation), which puts it on the circle a
/// round tessellation's corners lie on, to second order. An end's tangent planes are its corner
/// normals in the triangles beside the edge ([`corner_normals`]), each distinct one counted once:
/// one on a curved surface, the faces' own across a crease, where an edge along a flat face moves
/// not at all.
fn curved(mesh: &Mesh, mids: &FastMap<(u32, u32), u32>) -> Vec<(u32, Vec3)> {
    let topology = Topology::of(mesh);
    let smooth = vec![true; mesh.indices.len()];
    let corners = corner_normals(mesh, &topology.edges, &topology.faces, SMOOTH_CREASE, &smooth);
    let mut out = Vec::with_capacity(mids.len());
    for (&(a, b), &m) in mids {
        let Some(edge) = topology.edges.find(a, b) else { continue };
        let middle = (mesh.positions[a as usize] + mesh.positions[b as usize]) * 0.5;
        let mut bulge = Vec3::ZERO;
        for end in [a, b] {
            let mut planes: Vec<Vec3> = Vec::with_capacity(2);
            for &face in topology.edges.faces(edge) {
                let Some(k) = mesh.indices[face as usize].iter().position(|&v| v == end) else { continue };
                let normal = corners[face as usize][k];
                if normal != Vec3::ZERO && planes.iter().all(|seen| seen.dot(normal) < 1.0 - 1e-9) {
                    planes.push(normal);
                }
            }
            let to = middle - mesh.positions[end as usize];
            bulge = planes.iter().fold(bulge, |sum, &n| sum - n * to.dot(n));
        }
        out.push((m, bulge * 0.25));
    }
    out
}

/// One round of splitting by longest edges (Rivara's bisection): every triangle `pick` says yes to
/// (by its corners) whose longest edge is longer than `edge` has that edge split at its middle, and
/// so does every triangle an edge of which is split, so no triangle is ever cut across anything but
/// its longest side and none turns into a sliver. `None` when nothing is split, or when the split
/// would pass `budget` triangles. New vertices are numbered after the existing ones, in the order
/// their edges are first met, and each one's edge is returned in that order: the two vertices it
/// lies halfway between.
pub(crate) fn split_longest(
    mesh: &Mesh,
    pick: &dyn Fn([Vec3; 3]) -> bool,
    edge: f64,
    budget: usize,
) -> Option<(Mesh, Vec<[u32; 2]>)> {
    let longest = |tri: &[u32; 3]| {
        let length = |k: usize| (mesh.positions[tri[(k + 1) % 3] as usize] - mesh.positions[tri[k] as usize]).length();
        let k = (0..3).max_by(|&i, &j| length(i).total_cmp(&length(j))).expect("three sides");
        (key(tri[k], tri[(k + 1) % 3]), length(k))
    };
    let mut marked: FastMap<(u32, u32), u32> = FastMap::default();
    for tri in &mesh.indices {
        let (side, length) = longest(tri);
        if length > edge && pick(mesh.corners(*tri)) {
            marked.insert(side, 0);
        }
    }
    if marked.is_empty() {
        return None;
    }
    // A triangle with a split side has its longest side split too, until no more are added; each
    // added side is longer than the one that called for it, so this ends.
    loop {
        let mut added = false;
        for tri in &mesh.indices {
            let touched = (0..3).any(|k| marked.contains_key(&key(tri[k], tri[(k + 1) % 3])));
            let (side, _) = longest(tri);
            if touched && !marked.contains_key(&side) {
                marked.insert(side, 0);
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    // Each split side adds a triangle on each side of it.
    if mesh.indices.len() + 2 * marked.len() > budget {
        return None;
    }
    // Numbered in the order the sides are first met, so the result depends only on the mesh.
    let mut next_vertex = mesh.positions.len() as u32;
    let mut mids: FastMap<(u32, u32), u32> = FastMap::default();
    let mut parents = Vec::with_capacity(marked.len());
    for tri in &mesh.indices {
        for k in 0..3 {
            let side = key(tri[k], tri[(k + 1) % 3]);
            if marked.contains_key(&side) && !mids.contains_key(&side) {
                mids.insert(side, next_vertex);
                parents.push([side.0, side.1]);
                next_vertex += 1;
            }
        }
    }
    Some((split(mesh, &mids), parents))
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
