//! Which way, and how far, each vertex moves for its faces to move out by one unit: what inflating
//! needs, where the vertex normal is not enough.

use crate::mesh::Mesh;
use crate::topology::face_shape;
use crate::vec3::Vec3;

/// The most an offset vector may be as long as the distance its faces move, as a mitred join's
/// limit: a box corner needs sqrt(3), a needle's tip would need far more and is cut short.
const MITRE_LIMIT: f64 = 4.0;

/// Each vertex's offset vector `d`: moving it by `d * amount` moves every triangle around it out by
/// `amount` along the triangle's own normal, as nearly as one point can, so flat faces stay flat.
///
/// The unit vertex normal ([`crate::topology::vertex_normals`]) leans out of all the faces meeting at
/// a crease, so a box's edge vertices moved only `amount / sqrt(2)` out of each face and its corners
/// `amount / sqrt(3)`, leaving a 45-degree strip along every edge. Here `d` is the least-squares
/// solution of `d . n = 1` over the triangles' normals `n`, weighted by their angle at the vertex:
/// `(1, 1, 1)` at a box corner, `(1, 1, 0)` on an edge, the normal on a flat face and about the normal
/// on a curved one. Directions no normal spans (along a cylinder's axis) get nothing, which the
/// pseudo-inverse gives. `Vec3::ZERO` for a vertex with no triangle of any area.
pub(super) fn offset_directions(mesh: &Mesh) -> Vec<Vec3> {
    let mut sums = vec![([[0.0; 3]; 3], Vec3::ZERO); mesh.positions.len()];
    for (face, tri) in mesh.indices.iter().enumerate() {
        let (normal, angles) = face_shape(mesh, face as u32);
        if normal == Vec3::ZERO {
            continue;
        }
        let n = [normal.x, normal.y, normal.z];
        for k in 0..3 {
            let (a, b) = &mut sums[tri[k] as usize];
            for (i, row) in a.iter_mut().enumerate() {
                for (j, cell) in row.iter_mut().enumerate() {
                    *cell += angles[k] * n[i] * n[j];
                }
            }
            *b = *b + normal * angles[k];
        }
    }
    sums.into_iter().map(|(a, b)| solve(a, b)).collect()
}

/// The pseudo-inverse of the symmetric `a` applied to `b`, cut to [`MITRE_LIMIT`] long.
fn solve(a: [[f64; 3]; 3], b: Vec3) -> Vec3 {
    let (values, vectors) = eigen(a);
    let largest = values.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    if largest <= 0.0 {
        return Vec3::ZERO;
    }
    let mut d = Vec3::ZERO;
    for k in 0..3 {
        if values[k] > largest * 1e-6 {
            let v = Vec3::new(vectors[0][k], vectors[1][k], vectors[2][k]);
            d = d + v * (v.dot(b) / values[k]);
        }
    }
    let length = d.length();
    if length > MITRE_LIMIT {
        d * (MITRE_LIMIT / length)
    } else {
        d
    }
}

/// The eigenvalues of the symmetric `a` and its eigenvectors as columns, by Jacobi rotations.
fn eigen(mut a: [[f64; 3]; 3]) -> ([f64; 3], [[f64; 3]; 3]) {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..32 {
        let off = a[0][1].abs() + a[0][2].abs() + a[1][2].abs();
        if off <= 1e-15 * (a[0][0].abs() + a[1][1].abs() + a[2][2].abs()) {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q] == 0.0 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            // a = Pᵀ a P and v = v P, with P the rotation in the p-q plane.
            for row in a.iter_mut().chain(v.iter_mut()) {
                let (rp, rq) = (row[p], row[q]);
                row[p] = c * rp - s * rq;
                row[q] = s * rp + c * rq;
            }
            let (ap, aq) = (a[p], a[q]);
            a[p] = std::array::from_fn(|k| c * ap[k] - s * aq[k]);
            a[q] = std::array::from_fn(|k| s * ap[k] + c * aq[k]);
        }
    }
    ([a[0][0], a[1][1], a[2][2]], v)
}
