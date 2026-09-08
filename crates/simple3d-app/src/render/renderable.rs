//! What the renderer is handed: a body's triangles, and the edges worth
//! drawing on it.

use simple3d_geom::{Mesh, Vec3};
use std::collections::HashMap;

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
pub struct Renderable {
    pub mesh: Mesh,
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
}

impl Renderable {
    pub fn prepare(mesh: &Mesh) -> Renderable {
        Renderable::prepare_with(mesh, false)
    }

    /// The same, plus the edge adjacency the selection outline needs. For the
    /// nodes that may be drawn as a selection, which is a handful rather than
    /// the whole scene.
    pub fn prepare_outlined(mesh: &Mesh) -> Renderable {
        Renderable::prepare_with(mesh, true)
    }

    pub(super) fn prepare_with(mesh: &Mesh, outlined: bool) -> Renderable {
        let welded = mesh.weld();
        let edges = feature_edges(&welded, 20.0);
        let outline = if outlined { border_edges(&welded) } else { Vec::new() };
        let bodies = bodies_of(&welded);
        let body_count = bodies.iter().max().map_or(0, |last| last + 1);
        Renderable { mesh: welded, edges, outline, bodies, body_count }
    }

    pub fn empty() -> Renderable {
        Renderable { mesh: Mesh::new(), edges: Vec::new(), outline: Vec::new(), bodies: Vec::new(), body_count: 0 }
    }

    /// The body a triangle belongs to, as a tag for the depth buffer. `base` is
    /// where this item's bodies start, so two items never share a tag, and 0
    /// means "no body", so the numbering starts at 1.
    pub(super) fn tag(&self, triangle: usize, base: u16) -> u16 {
        self.mesh.indices.get(triangle).map_or(0, |tri| self.body_tag(tri[0] as usize, base))
    }

    pub(super) fn body_tag(&self, vertex: usize, base: u16) -> u16 {
        self.bodies.get(vertex).map_or(0, |body| base.saturating_add(*body).saturating_add(1))
    }
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
pub fn feature_edges(mesh: &Mesh, angle_deg: f64) -> Vec<[u32; 2]> {
    let cos_limit = angle_deg.to_radians().cos();
    let mut faces: HashMap<(u32, u32), Vec<Vec3>> = HashMap::new();
    for tri in &mesh.indices {
        let normal = mesh.triangle_normal(*tri);
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            let key = if a < b { (a, b) } else { (b, a) };
            faces.entry(key).or_default().push(normal);
        }
    }
    let mut edges: Vec<[u32; 2]> = faces
        .into_iter()
        .filter(|(_, normals)| match normals.as_slice() {
            [a, b] => a.dot(*b) < cos_limit,
            // One triangle (a boundary of an open mesh) or more than two (a
            // non-manifold junction): both are worth seeing.
            _ => true,
        })
        .map(|((a, b), _)| [a, b])
        .collect();
    // Deterministic order, so successive frames of an unchanged scene are
    // identical and the image comparison in the tests is meaningful.
    edges.sort_unstable();
    edges
}

/// Every edge of the mesh with the triangles that meet at it, in a
/// deterministic order.
pub(crate) fn border_edges(mesh: &Mesh) -> Vec<BorderEdge> {
    let mut faces: HashMap<(u32, u32), [u32; 2]> = HashMap::new();
    let mut counts: HashMap<(u32, u32), u32> = HashMap::new();
    for (index, tri) in mesh.indices.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (tri[k], tri[(k + 1) % 3]);
            let key = if a < b { (a, b) } else { (b, a) };
            let count = counts.entry(key).or_insert(0);
            let slot = faces.entry(key).or_insert([index as u32; 2]);
            if *count == 1 {
                slot[1] = index as u32;
            }
            // A third triangle on one edge is a non-manifold junction: leave the
            // first two, and let the count say it is not a plain edge.
            *count += 1;
        }
    }
    let mut out: Vec<BorderEdge> = faces
        .into_iter()
        .map(|(key, mut pair)| {
            let count = counts[&key];
            if count != 2 {
                // Always drawn: `push_selection` reads a repeated triangle as
                // "there is no far side to ask about".
                pair[1] = pair[0];
            }
            BorderEdge { ends: [key.0, key.1], faces: pair, junction: count > 2 }
        })
        .collect();
    out.sort_unstable_by_key(|edge| edge.ends);
    out
}
