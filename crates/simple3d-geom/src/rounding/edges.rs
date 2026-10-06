//! The sharp edges of a mesh, with the faces either side and whether they are convex.

use crate::mesh::{FastMap, Mesh};
use crate::vec3::Vec3;
use serde::{Deserialize, Serialize};

/// Faces meeting at more than this many degrees off flat make an edge; below it they are facets of
/// one curved surface.
pub const FEATURE_DEG: f64 = 25.0;

/// One straight sharp edge: two faces meeting along `a`-`b`. Stored by the rounding edits that keep
/// it (in the object's own frame), so it is serialised without its sources.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeatureEdge {
    pub a: Vec3,
    pub b: Vec3,
    /// The outward normals of the two faces.
    pub normals: [Vec3; 2],
    /// Each face's direction away from the edge, along the face and square to it.
    pub along: [Vec3; 2],
    /// The two faces' source nodes ([`Mesh::source`]).
    #[serde(skip)]
    pub sources: [u32; 2],
    /// Whether the solid's wedge between the faces is under 180 degrees: an outside edge, rounded by
    /// cutting, rather than an inside one, rounded by filling.
    pub convex: bool,
}

impl FeatureEdge {
    /// The angle of the wedge the two faces span on the side being worked: the solid's for a convex
    /// edge, the air's for a concave one. Always under 180 degrees.
    pub fn opening(&self) -> f64 {
        self.along[0].dot(self.along[1]).clamp(-1.0, 1.0).acos()
    }

    pub fn direction(&self) -> Vec3 {
        (self.b - self.a).normalized()
    }

    pub fn length(&self) -> f64 {
        (self.b - self.a).length()
    }
}

type Key = (i64, i64, i64);

fn key(p: Vec3) -> Key {
    let s = 1e5;
    ((p.x * s).round() as i64, (p.y * s).round() as i64, (p.z * s).round() as i64)
}

/// Every sharp edge of `mesh`, collinear pieces of one edge joined. Edges with other than two faces
/// (open or non-manifold ones) are left out.
pub fn feature_edges(mesh: &Mesh) -> Vec<FeatureEdge> {
    feature_edges_by(mesh, true)
}

/// [`feature_edges`], joining collinear pieces of different objects ([`Mesh::source`]) into one edge
/// only when `across_objects`; otherwise an edge running on from one object into the next is two.
pub fn feature_edges_by(mesh: &Mesh, across_objects: bool) -> Vec<FeatureEdge> {
    let keys: Vec<Key> = mesh.positions.iter().map(|&p| key(p)).collect();
    let mut at: FastMap<Key, Vec3> = FastMap::default();
    // Each directed edge and the triangle using it.
    let mut directed: FastMap<(Key, Key), Vec<usize>> = FastMap::default();
    for (t, tri) in mesh.indices.iter().enumerate() {
        for e in 0..3 {
            let (i, j) = (tri[e] as usize, tri[(e + 1) % 3] as usize);
            if keys[i] == keys[j] {
                continue;
            }
            at.insert(keys[i], mesh.positions[i]);
            directed.entry((keys[i], keys[j])).or_default().push(t);
        }
    }
    let cos_feature = FEATURE_DEG.to_radians().cos();
    let mut pieces = Vec::new();
    for (&(ka, kb), first) in directed.iter() {
        // Each undirected edge once, from the side its first triangle walks forwards.
        if ka > kb {
            continue;
        }
        let Some(second) = directed.get(&(kb, ka)) else { continue };
        let ([t1], [t2]) = (first.as_slice(), second.as_slice()) else { continue };
        let (n1, n2) = (mesh.triangle_normal(mesh.indices[*t1]), mesh.triangle_normal(mesh.indices[*t2]));
        if !(n1.length() > 0.5 && n2.length() > 0.5) || n1.dot(n2) > cos_feature {
            continue;
        }
        let (a, b) = (at[&ka], at[&kb]);
        let t = (b - a).normalized();
        let inward = |tri: usize, n: Vec3| {
            // Square to the edge, in the face, towards the face's far corner.
            let w = n.cross(t).normalized();
            let far = mesh.corners(mesh.indices[tri]).into_iter().fold(a, |best, p| {
                if ((p - a) - t * (p - a).dot(t)).length() > ((best - a) - t * (best - a).dot(t)).length() {
                    p
                } else {
                    best
                }
            });
            if w.dot(far - a) < 0.0 {
                -w
            } else {
                w
            }
        };
        let (w1, w2) = (inward(*t1, n1), inward(*t2, n2));
        // Convex when the second face turns away below the first face's plane.
        let convex = w2.dot(n1) < 0.0;
        pieces.push(FeatureEdge {
            a,
            b,
            normals: [n1, n2],
            along: [w1, w2],
            sources: [mesh.source(*t1), mesh.source(*t2)],
            convex,
        });
    }
    pieces.sort_by(|p, q| {
        let k = |e: &FeatureEdge| (key(e.a), key(e.b));
        k(p).cmp(&k(q))
    });
    join_collinear(pieces, across_objects)
}

/// Join pieces that continue one another: same faces, in line, meeting at a vertex no other edge
/// uses, and with `across_objects` off, on the same objects. Booleans split an edge wherever another
/// face's vertex touched it.
fn join_collinear(pieces: Vec<FeatureEdge>, across_objects: bool) -> Vec<FeatureEdge> {
    let mut ends: FastMap<Key, Vec<usize>> = FastMap::default();
    for (i, e) in pieces.iter().enumerate() {
        ends.entry(key(e.a)).or_default().push(i);
        ends.entry(key(e.b)).or_default().push(i);
    }
    let same_faces = |p: &FeatureEdge, q: &FeatureEdge| {
        let close = |u: Vec3, v: Vec3| u.dot(v) > 1.0 - 1e-9;
        (close(p.normals[0], q.normals[0]) && close(p.normals[1], q.normals[1]))
            || (close(p.normals[0], q.normals[1]) && close(p.normals[1], q.normals[0]))
    };
    let same_objects = |p: &FeatureEdge, q: &FeatureEdge| {
        let sorted =
            |e: &FeatureEdge| if e.sources[0] <= e.sources[1] { e.sources } else { [e.sources[1], e.sources[0]] };
        across_objects || sorted(p) == sorted(q)
    };
    let mut used = vec![false; pieces.len()];
    let mut out = Vec::new();
    for start in 0..pieces.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut edge = pieces[start];
        // Grow at either end while exactly one other piece continues it straight on.
        for forward in [true, false] {
            loop {
                let end = if forward { edge.b } else { edge.a };
                let Some(list) = ends.get(&key(end)) else { break };
                if list.len() != 2 {
                    break;
                }
                let Some(&next) = list.iter().find(|&&i| !used[i]) else { break };
                let other = pieces[next];
                let far = if key(other.a) == key(end) { other.b } else { other.a };
                let (dir, more) = (edge.direction(), (far - end).normalized());
                if !same_faces(&edge, &other) || !same_objects(&edge, &other) || dir.dot(more).abs() < 1.0 - 1e-9 {
                    break;
                }
                used[next] = true;
                if forward {
                    edge.b = far;
                } else {
                    edge.a = far;
                }
            }
        }
        out.push(edge);
    }
    out
}
