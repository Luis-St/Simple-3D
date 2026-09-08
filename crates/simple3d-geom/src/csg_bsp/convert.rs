//! Between meshes and the polygon soup the kernel works in.

use super::*;
use crate::mesh::Mesh;
use crate::vec3::Vec3;

pub fn debug_mesh_to_polygons(mesh: &Mesh) -> Vec<Vec<Vec3>> {
    mesh_to_polygons(mesh).into_iter().map(|p| p.vertices).collect()
}

/// What makes two triangles part of the same face: the plane they lie in and
/// the body they came from.
pub(crate) type FaceGroup = ((i64, i64, i64, i64), u32);

pub(crate) fn mesh_to_polygons(mesh: &Mesh) -> Vec<Polygon> {
    let planes: Vec<Option<Plane>> = mesh
        .indices
        .iter()
        .map(|t| {
            Plane::from_points(
                mesh.positions[t[0] as usize],
                mesh.positions[t[1] as usize],
                mesh.positions[t[2] as usize],
            )
        })
        .collect();
    // Grouped by plane *and* tag: coplanar faces of two different bodies are
    // not one face, and merging them would lose which body each part came from.
    let mut groups: std::collections::BTreeMap<FaceGroup, Vec<usize>> = std::collections::BTreeMap::new();
    for (i, p) in planes.iter().enumerate() {
        if let Some(pl) = p {
            groups.entry((plane_key(pl), mesh.tag(i))).or_default().push(i);
        }
    }
    let mut polygons = Vec::new();
    for (&(_, tag), tri_idxs) in groups.iter() {
        let plane = planes[tri_idxs[0]].unwrap();
        if tri_idxs.len() > 1 {
            if let Some(merged) = try_merge_group(mesh, tri_idxs, plane, tag) {
                polygons.push(merged);
                continue;
            }
        }
        for &i in tri_idxs {
            let t = mesh.indices[i];
            let (a, b, c) =
                (mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]);
            polygons.push(Polygon::new(vec![a, b, c], plane, tag));
        }
    }
    polygons
}

pub fn debug_roundtrip(mesh: &Mesh) -> Mesh {
    polygons_to_mesh(&mesh_to_polygons(mesh))
}

/// Whether `build` can chain this mesh's faces instead of classifying every one
/// of them against every plane -- true exactly when no face's plane divides
/// another, which is what a convex solid is. Exposed for the test that holds
/// the shortcut to that meaning.
pub fn debug_splits_nothing(mesh: &Mesh) -> bool {
    non_splitting_order(&mesh_to_polygons(mesh)).is_some()
}

pub(crate) fn polygons_to_mesh(polys: &[Polygon]) -> Mesh {
    let mut mesh = Mesh::new();
    for poly in polys {
        // Fan-triangulate; every polygon here is convex (a plane-clipped convex
        // input stays convex, and `try_merge_group` rejects concave merges), so
        // a fan from vertex 0 is always valid. Start the fan at a vertex that
        // actually turns: a merged face can carry collinear T-junction vertices,
        // and fanning from one of those emits zero-area triangles whose edges
        // then break the manifold check.
        let n = poly.vertices.len();
        if n < 3 {
            continue;
        }
        // A fan from vertex k produces a zero-area triangle whenever k lies on
        // the supporting line of one of the edges the fan spans. For a convex
        // loop that happens exactly when k is inside a collinear run or is
        // adjacent to a vertex that is, so require k and both its neighbours to
        // be genuine corners.
        let turns: Vec<bool> = (0..n)
            .map(|k| {
                let a = poly.vertices[(k + n - 1) % n];
                let b = poly.vertices[k];
                let c = poly.vertices[(k + 1) % n];
                (b - a).cross(c - b).length() > 1e-12
            })
            .collect();
        let apex = (0..n)
            .find(|&k| turns[k] && turns[(k + 1) % n] && turns[(k + n - 1) % n])
            .or_else(|| (0..n).find(|&k| turns[k]))
            .unwrap_or(0);
        for i in 1..n - 1 {
            mesh.push_tagged_triangle(
                poly.vertices[apex],
                poly.vertices[(apex + i) % n],
                poly.vertices[(apex + i + 1) % n],
                poly.tag,
            );
        }
    }
    mesh
}

/// If no plane of `polygons` divides any of them, the planes grouped by the
/// order they should be chained in; `None` if any plane splits something and
/// the general build must do the work.
///
/// This is the convex case, and it is not an exotic one: a sphere, a cylinder,
/// a box, a cap, a prism -- every round primitive the application offers is
/// convex, and a convex body is the worst case for the classic BSP build. No
/// face of a sphere divides the others (they are all behind it), so the tree is
/// a chain, and the general build re-classifies every remaining face at every
/// level to discover that: quadratic in the face count, and the reason a
/// spherical cap at 128 segments spent a second of its second inside
/// `BspNode::build` alone. Proving the same thing through the bounding-volume
/// hierarchy costs one query per distinct plane.
pub(crate) fn non_splitting_order(polygons: &[Polygon]) -> Option<Vec<Vec<usize>>> {
    /// Not worth the hierarchy: the general build is already linear here.
    const MIN: usize = 64;
    if polygons.len() < MIN {
        return None;
    }
    let tree = BoxTree::new(polygons)?;

    // Grouped by plane, in first-appearance order, so the chain takes the
    // planes in the order the general build would have taken them.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut index: std::collections::BTreeMap<(i64, i64, i64, i64), usize> = std::collections::BTreeMap::new();
    for (i, p) in polygons.iter().enumerate() {
        let slot = *index.entry(unoriented_plane_key(&p.plane)).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[slot].push(i);
    }
    for group in &groups {
        if !tree.all_behind(&polygons[group[0]].plane, polygons) {
            return None;
        }
    }
    Some(groups)
}
