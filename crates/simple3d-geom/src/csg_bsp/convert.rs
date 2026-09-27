//! Between meshes and the polygon soup the kernel works in.

use super::*;
use crate::mesh::Mesh;
use crate::vec3::Vec3;

pub fn debug_mesh_to_polygons(mesh: &Mesh) -> Vec<Vec<Vec3>> {
    mesh_to_polygons(mesh).into_iter().map(|p| p.vertices).collect()
}

/// What makes two triangles one face: their plane and their body.
pub(crate) type FaceGroup = ((i64, i64, i64, i64), u32);

pub(crate) fn mesh_to_polygons(mesh: &Mesh) -> Vec<Polygon> {
    let planes: Vec<Option<Plane>> = mesh
        .indices
        .iter()
        .map(|t| {
            let [a, b, c] = mesh.corners(*t);
            Plane::from_points(a, b, c)
        })
        .collect();
    // Grouped by plane and tag, so coplanar faces of different bodies keep their origin.
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
            let [a, b, c] = mesh.corners(t);
            polygons.push(Polygon::new(vec![a, b, c], plane, tag));
        }
    }
    polygons
}

pub fn debug_roundtrip(mesh: &Mesh) -> Mesh {
    polygons_to_mesh(&mesh_to_polygons(mesh))
}

/// Whether `build` can chain this mesh's faces: no face plane divides another (a convex solid).
/// Exposed for the test pinning that meaning.
pub fn debug_splits_nothing(mesh: &Mesh) -> bool {
    non_splitting_order(&mesh_to_polygons(mesh)).is_some()
}

pub(crate) fn polygons_to_mesh(polys: &[Polygon]) -> Mesh {
    let mut mesh = Mesh::new();
    for poly in polys {
        // Fan-triangulate convex polygons, starting at a real corner: fanning from a collinear
        // T-junction vertex emits zero-area triangles that break the manifold check.
        let n = poly.vertices.len();
        if n < 3 {
            continue;
        }
        // Require the fan vertex and both neighbours to be real corners, or the fan makes zero-area triangles.
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

/// If no plane of `polygons` divides any of them, the planes grouped in chain order; `None` if the
/// general build is needed. Every round primitive is convex, the classic build's quadratic worst
/// case; proving it via the box hierarchy costs one query per plane.
pub(crate) fn non_splitting_order(polygons: &[Polygon]) -> Option<Vec<Vec<usize>>> {
    /// Below this the general build is already cheap.
    const MIN: usize = 64;
    if polygons.len() < MIN {
        return None;
    }
    let tree = BoxTree::new(polygons)?;

    // Grouped by plane in first-appearance order, as the general build would take them.
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
