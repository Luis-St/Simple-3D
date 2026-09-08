//! The watertightness check that runs before anything is written, so a
//! model a slicer would refuse is refused here first.

use simple3d_geom::Mesh;

/// Check a mesh is fit to write: closed, edge-manifold, consistently wound, and
/// with normals facing outward. Returns one description per problem found.
pub fn verify(mesh: &Mesh) -> Vec<String> {
    let mut problems = Vec::new();
    if mesh.triangle_count() == 0 {
        problems.push("the mesh has no triangles".to_string());
        return problems;
    }
    if let Some(issue) = mesh.manifold_issue() {
        problems.push(format!("the mesh is not watertight and manifold ({issue})"));
    }
    let volume = signed_volume(mesh);
    if volume <= 0.0 {
        problems.push(format!(
            "the triangles are wound inward (signed volume {volume:.3} mm3); normals would point into the solid"
        ));
    }
    for (i, tri) in mesh.indices.iter().enumerate() {
        let normal = mesh.triangle_normal(*tri);
        if normal.length() < 0.5 {
            problems.push(format!("triangle {i} has no area, so it has no normal"));
            break;
        }
    }
    problems
}

/// Six times the signed volume is the sum of the triangles' scalar triple
/// products; positive means the winding is outward for a closed mesh.
pub fn signed_volume(mesh: &Mesh) -> f64 {
    let mut total = 0.0;
    for tri in &mesh.indices {
        let a = mesh.positions[tri[0] as usize];
        let b = mesh.positions[tri[1] as usize];
        let c = mesh.positions[tri[2] as usize];
        total += a.dot(b.cross(c));
    }
    total / 6.0
}
