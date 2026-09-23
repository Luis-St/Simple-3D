//! What the renderer is handed: a body's triangles, and the edges worth
//! drawing on it.

use super::{axis_inside_spans, map_in_order};
use simple3d_core::scene::NodeId;
use simple3d_geom::{Mesh, Vec3};
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::OnceLock;

/// An edge of the surface with the triangles that meet at it.
///
/// What the selection outline is drawn from. Which edges make up a shape's
/// outline depends on where the camera is -- an edge is on the silhouette when
/// the surface turns away from the eye across it -- so it cannot be settled
/// once at preparation time the way a crease can.
pub struct BorderEdge {
    pub ends: [u32; 2],
    /// The two triangles either side of it, or the same one twice when the edge
    /// has only one -- an open boundary. Those are drawn whatever the camera is
    /// doing: there is no surface on the far side for the silhouette test to
    /// ask about.
    pub faces: [u32; 2],
    /// More than two triangles meet along this edge, which is what happens
    /// where two bodies of the same mesh touch: the seam between two pieces of
    /// a split has each piece's own face and the face they share. It is inside
    /// the shape, not on its outline, and drawing it lit every cut of an
    /// eighty-piece split as a cage over the model.
    pub junction: bool,
}

/// A mesh prepared for drawing: welded, so edges can be found, together with
/// its feature edges.
///
/// Everything here that does not depend on where the camera is standing is
/// worked out once, when the renderable is made, rather than once per frame:
/// the face normals, how far the mesh reaches from the origin, and where each
/// origin axis runs through it. A renderable is rebuilt only when the
/// evaluation or the selection changes, so an orbit re-uses all of it, and the
/// alternative was re-deriving it from a hundred and fifty thousand triangles
/// for every frame of the drag.
pub struct Renderable {
    pub mesh: Mesh,
    /// One outward unit normal per triangle, in the order `mesh.indices` has
    /// them. `Vec3::ZERO` for a triangle too degenerate to have one, which is
    /// exactly what [`Mesh::triangle_normal`] answers for it, so a caller tests
    /// for that rather than measuring the cross product itself.
    pub normals: Vec<Vec3>,
    /// Edges worth drawing: a real crease in the surface, not an artefact of how
    /// a flat face happens to be triangulated.
    pub edges: Vec<[u32; 2]>,
    /// Every edge of the surface, with its neighbours -- what the silhouette is
    /// picked out of each frame.
    ///
    /// Empty unless the item may be drawn as a selection: it is one entry per
    /// edge rather than per crease, which for a large mesh is millions, and the
    /// scene as a whole is never outlined.
    pub outline: Vec<BorderEdge>,
    /// Which separate body each vertex belongs to: two vertices share a number
    /// when the surface joins them. The viewport hands the renderer the whole
    /// evaluated scene as *one* mesh, so this is the only thing that says where
    /// one solid ends and the next begins -- and an origin axis has to know,
    /// because the solid it runs into may not hide it while every other one
    /// must (issue 47).
    pub bodies: Vec<u16>,
    /// How many bodies that is: what the next item's tags start after, so two
    /// items' bodies are never the same body as far as the frame is concerned.
    pub body_count: u16,
    /// The distance from the origin to the furthest vertex: what an origin
    /// axis's arms have to be longer than (see `AxisMaterial::reach`).
    pub reach: f64,
    /// Per axis, the stretches of that axis that run inside this mesh, each
    /// with the *body* it runs through -- not the tag, because the tag depends
    /// on where this item's bodies start in the frame and the spans do not.
    pub(super) axis_spans: [Vec<((f64, f64), u16)>; 3],
    /// Per principal plane, numbered by the axis it is perpendicular to, the
    /// segments where it crosses the surface: the plane marks, which used to be
    /// found anew on every frame from every triangle, three times over.
    ///
    /// Found the first time the software renderer asks for them, and only
    /// then: the GPU finds the marks on the card and never does.
    plane_marks: OnceLock<[Vec<[Vec3; 2]>; 3]>,
    /// Which welded vertices are each node's, for every node whose geometry
    /// is a stretch of this mesh of its own -- untouched by any boolean, see
    /// `Evaluated::ranges`, and sharing no vertex with anything else. A node's
    /// triangles and edges are exactly the ones that use its vertices, which
    /// is what lets the GPU leave a node out of the picture while it is being
    /// dragged and draw it where the drag has got to instead.
    ///
    /// Empty for anything but the whole scene.
    pub parts: BTreeMap<NodeId, Range<u32>>,
    /// Which renderable this is, unique for the life of the process. What the
    /// GPU renderer keys the copy of the mesh it keeps on the card by: the
    /// renderable never changes once made, so as long as the same one is
    /// handed in, the geometry already uploaded for it is still the geometry.
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

    /// The whole evaluated scene, with where each node that came through the
    /// evaluation untouched is in it: `ranges` is `Evaluated::ranges`, in
    /// the scene mesh's own vertices, and comes out as [`Renderable::parts`].
    pub fn prepare_scene(mesh: &Mesh, ranges: &BTreeMap<NodeId, Range<u32>>) -> Renderable {
        Renderable::prepare_with(mesh, false, ranges)
    }

    /// The same, plus the edge adjacency the selection outline needs. For the
    /// nodes that may be drawn as a selection, which is a handful rather than
    /// the whole scene.
    pub fn prepare_outlined(mesh: &Mesh) -> Renderable {
        Renderable::prepare_with(mesh, true, &BTreeMap::new())
    }

    pub(super) fn prepare_with(mesh: &Mesh, outlined: bool, ranges: &BTreeMap<NodeId, Range<u32>>) -> Renderable {
        let (welded, remap) = mesh.weld_with_remap();
        let normals: Vec<Vec3> =
            map_in_order(welded.indices.len(), |index| welded.triangle_normal(welded.indices[index]));
        // The rest is independent work over the same welded mesh, so it runs
        // side by side: on a large import each part is a pass over millions of
        // triangles, and one after another they were most of a second.
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

    /// Lines with no surface under them -- a tool's preview loops -- for the
    /// GPU to keep on the card like any mesh's edges.
    pub(crate) fn lines(positions: Vec<Vec3>, edges: Vec<[u32; 2]>) -> Renderable {
        let bodies = vec![0; positions.len()];
        Renderable {
            mesh: Mesh { positions, indices: Vec::new(), tags: Vec::new() },
            bodies,
            edges,
            ..Renderable::empty()
        }
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

    /// The body a triangle belongs to, as a tag for the depth buffer. `base` is
    /// where this item's bodies start, so two items never share a tag, and 0
    /// means "no body", so the numbering starts at 1.
    pub(super) fn tag(&self, triangle: usize, base: u16) -> u16 {
        self.mesh.indices.get(triangle).map_or(0, |tri| self.body_tag(tri[0] as usize, base))
    }

    pub(super) fn body_tag(&self, vertex: usize, base: u16) -> u16 {
        self.bodies.get(vertex).map_or(0, |body| body_tag(*body, base))
    }

    /// The plane marks -- see the field.
    pub(crate) fn plane_marks(&self) -> &[Vec<[Vec3; 2]>; 3] {
        self.plane_marks.get_or_init(|| std::array::from_fn(|axis| plane_marks_of(&self.mesh, axis)))
    }
}

/// Which welded vertices are each node's -- [`Renderable::parts`] -- from
/// where the weld sent each of the mesh's own vertices and which of those
/// were each node's.
///
/// Welded vertices are numbered in the order their first copy appears, so a
/// node whose vertices are a stretch of the mesh and are shared with nothing
/// outside it welds to a stretch as well: the ones first seen inside its own.
/// It is kept when that holds -- nothing it welds to was seen before it began
/// or is used again after it ends -- and dropped when it does not, which is
/// what two bodies laid side by side and touching do.
fn parts_of(remap: &[u32], welded: usize, ranges: &BTreeMap<NodeId, Range<u32>>) -> BTreeMap<NodeId, Range<u32>> {
    if ranges.is_empty() {
        return BTreeMap::new();
    }
    // For each welded vertex, the first and the last of the mesh's vertices
    // that went into it.
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
        // The first copy of every one of them is inside the range -- first
        // copies come in order, so the lowest and the highest decide -- and
        // no copy of any of them comes after it.
        let owned = first[lo] >= range.start && first[hi - 1] < range.end;
        if owned && last[lo..hi].iter().all(|&at| at < range.end) {
            parts.insert(id, lo as u32..hi as u32);
        }
    }
    parts
}

/// The depth-buffer tag a body of one item carries, where `base` is where that
/// item's bodies start. Shared, because a span cached on the renderable knows
/// its body but cannot know the base, and the two have to agree.
pub(crate) fn body_tag(body: u16, base: u16) -> u16 {
    base.saturating_add(body).saturating_add(1)
}

/// Where the plane perpendicular to `axis` through the origin crosses the
/// surface, one segment per triangle it runs through, in triangle order.
fn plane_marks_of(mesh: &Mesh, axis: usize) -> Vec<[Vec3; 2]> {
    mesh.indices
        .iter()
        .filter_map(|tri| {
            let world = tri.map(|corner| mesh.positions[corner as usize]);
            crate::snap::plane_crossing(world, axis).map(|(a, b)| [a, b])
        })
        .collect()
}

/// Group the vertices of a welded mesh into connected bodies: union-find over
/// the triangles, which is what "one solid" means once the scene has been
/// evaluated into a single mesh.
///
/// More than `u16::MAX` bodies would be a scene of sixty-five thousand separate
/// solids; past that they share the last number, which costs nothing but the
/// distinction between two axes' worth of far-off shapes.
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

/// Edges where the surface actually creases, plus any edge with only one
/// triangle. Drawing *every* triangle edge would cover a cylinder in meridians
/// and a boolean result in the arbitrary cuts the BSP made across flat faces --
/// noise rather than information.
///
/// The tests' way in: a renderable finds its edges from the normals it has
/// already worked out.
#[cfg(test)]
pub fn feature_edges(mesh: &Mesh, angle_deg: f64) -> Vec<[u32; 2]> {
    let normals: Vec<Vec3> = mesh.indices.iter().map(|tri| mesh.triangle_normal(*tri)).collect();
    EdgeTable::of(mesh).feature_edges(&normals, angle_deg)
}

/// Every edge of the mesh with the triangles that meet at it, in a
/// deterministic order.
#[cfg(test)]
pub(crate) fn border_edges(mesh: &Mesh) -> Vec<BorderEdge> {
    EdgeTable::of(mesh).border_edges()
}

/// Every edge of a mesh with the triangles that meet at it, grouped by edge.
///
/// Laid out by the edge's lower vertex, the way a sparse matrix is stored by
/// row: `entries[start[v]..start[v + 1]]` holds, for every edge whose lower
/// end is `v`, its upper end and one triangle along it, sorted. The entries of
/// one edge are then side by side, and walking the table visits the edges in
/// order of their ends -- the order both callers had sorted their output into.
///
/// It replaces a hash map from edge to triangles, which on an import of a
/// million and a half triangles took most of a second to build and twice that
/// when the outline needed a second one. A vertex has half a dozen edges, so
/// each row sorts in a handful of comparisons and the whole table is a few
/// passes over the triangles.
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

    /// Each edge in order, with the triangles along it in ascending order.
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
                // One triangle (a boundary of an open mesh) or more than two (a
                // non-manifold junction): both are worth seeing.
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
            // A third triangle on one edge is a non-manifold junction; anything
            // but two is always drawn, which `push_selection` reads off a
            // repeated triangle as "there is no far side to ask about".
            let pair = match faces {
                [(_, a), (_, b)] => [*a, *b],
                _ => [faces[0].1; 2],
            };
            out.push(BorderEdge { ends, faces: pair, junction: faces.len() > 2 });
        });
        out
    }
}
