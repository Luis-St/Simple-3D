//! What the renderer is handed: a body's triangles and the edges worth drawing.

use super::{axis_inside_spans, map_in_order};
use simple3d_core::scene::NodeId;
use simple3d_geom::{Mesh, Vec3};
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::OnceLock;

/// A surface edge with the triangles that meet at it, for the camera-dependent selection outline.
pub struct BorderEdge {
    pub ends: [u32; 2],
    /// The triangles either side, or the same one twice for an open boundary edge, which is
    /// always drawn.
    pub faces: [u32; 2],
    /// More than two triangles meet here, as where two bodies of one mesh touch. Inside the shape,
    /// so not outlined; drawing it lit every cut of a split as a cage over the model.
    pub junction: bool,
}

/// A welded mesh prepared for drawing, with its feature edges.
///
/// Everything camera-independent (normals, reach, axis spans) is computed once here, so an
/// orbit reuses it instead of re-deriving it from every triangle each frame.
pub struct Renderable {
    pub mesh: Mesh,
    /// One outward unit normal per triangle; `Vec3::ZERO` for a degenerate one, as
    /// [`Mesh::triangle_normal`] gives.
    pub normals: Vec<Vec3>,
    /// Real creases, not artefacts of how flat faces are triangulated.
    pub edges: Vec<[u32; 2]>,
    /// Every surface edge with its neighbours, for the per-frame silhouette. Empty unless the item
    /// may be drawn as a selection, since this can be millions of entries.
    pub outline: Vec<BorderEdge>,
    /// The connected body of each vertex. The scene is one mesh, so this is the only way to tell
    /// solids apart, which the origin axis needs (issue 47).
    pub bodies: Vec<u16>,
    /// Number of bodies, so the next item's tags start after them.
    pub body_count: u16,
    /// Distance from the origin to the furthest vertex (see `AxisMaterial::reach`).
    pub reach: f64,
    /// Per axis, the stretches inside this mesh with the body they run through. Bodies rather than
    /// tags, since tags depend on the item's base in the frame.
    pub(super) axis_spans: [Vec<((f64, f64), u16)>; 3],
    /// Per principal plane (by perpendicular axis), the segments where it crosses the surface.
    /// Computed lazily for the software renderer; the GPU finds them on the card.
    plane_marks: OnceLock<[Vec<[Vec3; 2]>; 3]>,
    /// The welded vertex range of each node whose geometry is its own untouched, unshared stretch
    /// (`Evaluated::ranges`), letting the GPU hide a dragged node and draw it moved instead.
    /// Empty for anything but the whole scene.
    pub parts: BTreeMap<NodeId, Range<u32>>,
    /// Unique per process; the GPU renderer keys its uploaded copy by it, since a renderable never
    /// changes once made.
    pub(crate) id: u64,
}

/// The next [`Renderable::id`].
fn next_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Renderable {
    pub fn prepare(mesh: &Mesh) -> Renderable {
        Renderable::prepare_with(mesh, false, &BTreeMap::new())
    }

    /// The whole evaluated scene; `ranges` (`Evaluated::ranges`) become [`Renderable::parts`].
    pub fn prepare_scene(mesh: &Mesh, ranges: &BTreeMap<NodeId, Range<u32>>) -> Renderable {
        Renderable::prepare_with(mesh, false, ranges)
    }

    /// The same, plus the edge adjacency the selection outline needs.
    pub fn prepare_outlined(mesh: &Mesh) -> Renderable {
        Renderable::prepare_with(mesh, true, &BTreeMap::new())
    }

    pub(super) fn prepare_with(mesh: &Mesh, outlined: bool, ranges: &BTreeMap<NodeId, Range<u32>>) -> Renderable {
        let (welded, remap) = mesh.weld_with_remap();
        let normals: Vec<Vec3> =
            map_in_order(welded.indices.len(), |index| welded.triangle_normal(welded.indices[index]));
        // Independent passes over the welded mesh run in parallel: sequentially they took most of a
        // second on a large import.
        let (table, bodies, parts) = std::thread::scope(|scope| {
            let table = scope.spawn(|| EdgeTable::of(&welded));
            let parts = scope.spawn(|| parts_of(&remap, welded.positions.len(), ranges));
            let bodies = bodies_of(&welded);
            (table.join().expect("the edge table panicked"), bodies, parts.join().expect("the parts panicked"))
        });
        let (edges, outline, axis_spans) = std::thread::scope(|scope| {
            let outline = scope.spawn(|| if outlined { table.border_edges() } else { Vec::new() });
            let spans = scope.spawn(|| std::array::from_fn(|axis| axis_inside_spans(&welded, &bodies, axis)));
            let edges = table.feature_edges(&normals, 20.0);
            (edges, outline.join().expect("the outline panicked"), spans.join().expect("the axis spans panicked"))
        });
        let body_count = bodies.iter().max().map_or(0, |last| last + 1);
        let reach = welded.positions.iter().map(|p| p.length()).fold(0.0, f64::max);
        Renderable {
            mesh: welded,
            normals,
            edges,
            outline,
            bodies,
            body_count,
            reach,
            axis_spans,
            plane_marks: OnceLock::new(),
            parts,
            id: next_id(),
        }
    }

    /// Lines with no surface, such as a tool's preview loops, kept on the card like mesh edges.
    pub(crate) fn lines(positions: Vec<Vec3>, edges: Vec<[u32; 2]>) -> Renderable {
        let bodies = vec![0; positions.len()];
        Renderable {
            mesh: Mesh { positions, indices: Vec::new(), tags: Vec::new() },
            bodies,
            edges,
            ..Renderable::empty()
        }
    }

    /// Only a surface (no weld, edges or bodies): a boolean operand drawn on the card while dragged
    /// (`App::live_csg`).
    pub(crate) fn surface(mesh: Mesh) -> Renderable {
        let bodies = vec![0; mesh.positions.len()];
        Renderable { mesh, bodies, ..Renderable::empty() }
    }

    /// [`Renderable::surface`] plus feature edges, for when the model's lines are drawn.
    pub(crate) fn surface_with_edges(mesh: &Mesh) -> Renderable {
        let (welded, _) = mesh.weld_with_remap();
        let normals: Vec<Vec3> =
            map_in_order(welded.indices.len(), |index| welded.triangle_normal(welded.indices[index]));
        let edges = EdgeTable::of(&welded).feature_edges(&normals, 20.0);
        let bodies = vec![0; welded.positions.len()];
        Renderable { mesh: welded, normals, edges, bodies, ..Renderable::empty() }
    }

    pub fn empty() -> Renderable {
        Renderable {
            mesh: Mesh::new(),
            normals: Vec::new(),
            edges: Vec::new(),
            outline: Vec::new(),
            bodies: Vec::new(),
            body_count: 0,
            reach: 0.0,
            axis_spans: [Vec::new(), Vec::new(), Vec::new()],
            plane_marks: OnceLock::new(),
            parts: BTreeMap::new(),
            id: next_id(),
        }
    }

    /// The depth-buffer tag of a triangle's body; `base` offsets per item, and 0 means no body.
    pub(super) fn tag(&self, triangle: usize, base: u16) -> u16 {
        self.mesh.indices.get(triangle).map_or(0, |tri| self.body_tag(tri[0] as usize, base))
    }

    pub(super) fn body_tag(&self, vertex: usize, base: u16) -> u16 {
        self.bodies.get(vertex).map_or(0, |body| body_tag(*body, base))
    }

    /// The plane marks (see the field).
    pub(crate) fn plane_marks(&self) -> &[Vec<[Vec3; 2]>; 3] {
        self.plane_marks.get_or_init(|| std::array::from_fn(|axis| plane_marks_of(&self.mesh, axis)))
    }
}

/// Which welded vertices are each node's ([`Renderable::parts`]).
///
/// Welded vertices are numbered by first appearance, so an unshared node range welds to a
/// contiguous range. A node is dropped when its vertices weld to ones outside it, as with two
/// touching bodies.
fn parts_of(remap: &[u32], welded: usize, ranges: &BTreeMap<NodeId, Range<u32>>) -> BTreeMap<NodeId, Range<u32>> {
    if ranges.is_empty() {
        return BTreeMap::new();
    }
    // For each welded vertex, the first and last mesh vertices that went into it.
    let mut first = vec![u32::MAX; welded];
    let mut last = vec![0u32; welded];
    for (index, &to) in remap.iter().enumerate() {
        let to = to as usize;
        first[to] = first[to].min(index as u32);
        last[to] = index as u32;
    }
    let mut parts = BTreeMap::new();
    for (&id, range) in ranges {
        let (start, end) = (range.start as usize, range.end as usize);
        if start >= end || end > remap.len() {
            continue;
        }
        let lo = remap[start..end].iter().copied().min().unwrap_or(0) as usize;
        let hi = remap[start..end].iter().copied().max().unwrap_or(0) as usize + 1;
        // First copies come in order, so the lowest and highest decide; no copy may come after the range.
        let owned = first[lo] >= range.start && first[hi - 1] < range.end;
        if owned && last[lo..hi].iter().all(|&at| at < range.end) {
            parts.insert(id, lo as u32..hi as u32);
        }
    }
    parts
}

/// The depth-buffer tag of an item's body. Shared so cached spans and draws agree on it.
pub(crate) fn body_tag(body: u16, base: u16) -> u16 {
    base.saturating_add(body).saturating_add(1)
}

/// Where the plane perpendicular to `axis` through the origin crosses the surface, per triangle.
fn plane_marks_of(mesh: &Mesh, axis: usize) -> Vec<[Vec3; 2]> {
    mesh.indices
        .iter()
        .filter_map(|tri| {
            let world = tri.map(|corner| mesh.positions[corner as usize]);
            crate::snap::plane_crossing(world, axis).map(|(a, b)| [a, b])
        })
        .collect()
}

/// Group a welded mesh's vertices into connected bodies by union-find over the triangles.
/// Past `u16::MAX` bodies they share the last number.
pub(crate) fn bodies_of(mesh: &Mesh) -> Vec<u16> {
    let mut parent: Vec<u32> = (0..mesh.positions.len() as u32).collect();
    fn find(parent: &mut [u32], mut of: u32) -> u32 {
        while parent[of as usize] != of {
            parent[of as usize] = parent[parent[of as usize] as usize];
            of = parent[of as usize];
        }
        of
    }
    for tri in &mesh.indices {
        let root = find(&mut parent, tri[0]);
        for &vertex in &tri[1..] {
            let other = find(&mut parent, vertex);
            if other != root {
                parent[other as usize] = root;
            }
        }
    }
    let mut numbers: std::collections::HashMap<u32, u16> = std::collections::HashMap::new();
    (0..mesh.positions.len() as u32)
        .map(|vertex| {
            let root = find(&mut parent, vertex);
            let next = numbers.len().min(u16::MAX as usize - 1) as u16;
            *numbers.entry(root).or_insert(next)
        })
        .collect()
}

/// Real creases plus single-triangle edges; every triangle edge would just be noise.
/// Test entry point.
#[cfg(test)]
pub fn feature_edges(mesh: &Mesh, angle_deg: f64) -> Vec<[u32; 2]> {
    let normals: Vec<Vec3> = mesh.indices.iter().map(|tri| mesh.triangle_normal(*tri)).collect();
    EdgeTable::of(mesh).feature_edges(&normals, angle_deg)
}

/// Every edge with the triangles that meet at it, in a deterministic order.
#[cfg(test)]
pub(crate) fn border_edges(mesh: &Mesh) -> Vec<BorderEdge> {
    EdgeTable::of(mesh).border_edges()
}

/// Every edge of a mesh with its triangles, grouped by the edge's lower vertex like a sparse
/// matrix row: `entries[start[v]..start[v + 1]]` holds (upper end, triangle), sorted.
///
/// Replaces a hash map that took most of a second on a 1.5M-triangle import.
struct EdgeTable {
    start: Vec<u32>,
    /// Upper end, then triangle.
    entries: Vec<(u32, u32)>,
}

impl EdgeTable {
    fn of(mesh: &Mesh) -> EdgeTable {
        let lower = |tri: &[u32; 3], k: usize| {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            if a < b {
                (a, b)
            } else {
                (b, a)
            }
        };
        let mut start = vec![0u32; mesh.positions.len() + 1];
        for tri in &mesh.indices {
            for k in 0..3 {
                start[lower(tri, k).0 as usize + 1] += 1;
            }
        }
        for v in 0..mesh.positions.len() {
            start[v + 1] += start[v];
        }
        let mut cursor = start.clone();
        let mut entries = vec![(0u32, 0u32); mesh.indices.len() * 3];
        for (face, tri) in mesh.indices.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = lower(tri, k);
                entries[cursor[a as usize] as usize] = (b, face as u32);
                cursor[a as usize] += 1;
            }
        }
        for v in 0..mesh.positions.len() {
            entries[start[v] as usize..start[v + 1] as usize].sort_unstable();
        }
        EdgeTable { start, entries }
    }

    /// Each edge in order, with its triangles in ascending order.
    fn for_each(&self, mut f: impl FnMut([u32; 2], &[(u32, u32)])) {
        for a in 0..self.start.len().saturating_sub(1) {
            let row = &self.entries[self.start[a] as usize..self.start[a + 1] as usize];
            for run in row.chunk_by(|x, y| x.0 == y.0) {
                f([a as u32, run[0].0], run);
            }
        }
    }

    fn feature_edges(&self, normals: &[Vec3], angle_deg: f64) -> Vec<[u32; 2]> {
        let cos_limit = angle_deg.to_radians().cos();
        let mut edges = Vec::new();
        self.for_each(|ends, faces| {
            let keep = match faces {
                [(_, a), (_, b)] => normals[*a as usize].dot(normals[*b as usize]) < cos_limit,
                // A boundary edge or a non-manifold junction: both are worth seeing.
                _ => true,
            };
            if keep {
                edges.push(ends);
            }
        });
        edges
    }

    fn border_edges(&self) -> Vec<BorderEdge> {
        let mut out = Vec::new();
        self.for_each(|ends, faces| {
            // Anything but two faces is always drawn; `push_selection` reads a repeated triangle as
            // "no far side".
            let pair = match faces {
                [(_, a), (_, b)] => [*a, *b],
                _ => [faces[0].1; 2],
            };
            out.push(BorderEdge { ends, faces: pair, junction: faces.len() > 2 });
        });
        out
    }
}
