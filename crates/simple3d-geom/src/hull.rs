//! 3D convex hull via randomized incremental construction (Berg et al.,
//! "Computational Geometry", ch. 11). Used for the group Hull operation.

use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::collections::BTreeMap;

struct Face {
    v: [usize; 3],
    normal: Vec3,
}

fn face_normal(pts: &[Vec3], v: [usize; 3]) -> Vec3 {
    let (a, b, c) = (pts[v[0]], pts[v[1]], pts[v[2]]);
    (b - a).cross(c - a).normalized()
}

pub fn convex_hull(points: &[Vec3]) -> Mesh {
    let mut pts: Vec<Vec3> = Vec::with_capacity(points.len());
    for p in points {
        if !pts.iter().any(|q: &Vec3| (*q - *p).length() < 1e-9) {
            pts.push(*p);
        }
    }
    if pts.len() < 4 {
        return Mesh::new();
    }

    let bounds_diag = {
        let (lo, hi) = (pts.iter().cloned().fold(pts[0], Vec3::min), pts.iter().cloned().fold(pts[0], Vec3::max));
        (hi - lo).length().max(1.0)
    };
    let eps = bounds_diag * 1e-9;

    // Initial tetrahedron: extreme point, farthest from it, farthest from that
    // line, farthest from that plane.
    let p0 = 0usize;
    let p1 = (0..pts.len())
        .max_by(|&a, &b| (pts[a] - pts[p0]).length().partial_cmp(&(pts[b] - pts[p0]).length()).unwrap())
        .unwrap();
    let dir = (pts[p1] - pts[p0]).normalized();
    let p2 = (0..pts.len())
        .max_by(|&a, &b| {
            let da = pts[a] - pts[p0];
            let db = pts[b] - pts[p0];
            let pa = (da - dir * da.dot(dir)).length();
            let pb = (db - dir * db.dot(dir)).length();
            pa.partial_cmp(&pb).unwrap()
        })
        .unwrap();
    let plane_n = (pts[p1] - pts[p0]).cross(pts[p2] - pts[p0]);
    if plane_n.length() < eps {
        // Degenerate (collinear) input; nothing sane to hull.
        return Mesh::new();
    }
    let plane_n = plane_n.normalized();
    let p3 = (0..pts.len())
        .max_by(|&a, &b| {
            let da = (pts[a] - pts[p0]).dot(plane_n).abs();
            let db = (pts[b] - pts[p0]).dot(plane_n).abs();
            da.partial_cmp(&db).unwrap()
        })
        .unwrap();
    if (pts[p3] - pts[p0]).dot(plane_n).abs() < eps {
        // All points coplanar; a proper hull would be a flat polygon. Not a
        // solid, so we return nothing rather than a degenerate zero-volume mesh.
        return Mesh::new();
    }

    let centroid = (pts[p0] + pts[p1] + pts[p2] + pts[p3]) * 0.25;
    let mut faces: Vec<Face> = Vec::new();
    let add_face = |pts: &[Vec3], v: [usize; 3], faces: &mut Vec<Face>| {
        let mut n = face_normal(pts, v);
        let mut v = v;
        if n.dot(pts[v[0]] - centroid) < 0.0 {
            v.swap(1, 2);
            n = -n;
        }
        faces.push(Face { v, normal: n });
    };
    add_face(&pts, [p0, p1, p2], &mut faces);
    add_face(&pts, [p0, p1, p3], &mut faces);
    add_face(&pts, [p0, p2, p3], &mut faces);
    add_face(&pts, [p1, p2, p3], &mut faces);

    let used: [usize; 4] = [p0, p1, p2, p3];
    for (i, p) in pts.iter().enumerate() {
        if used.contains(&i) {
            continue;
        }
        let visible: Vec<usize> = faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.normal.dot(*p - pts[f.v[0]]) > eps)
            .map(|(idx, _)| idx)
            .collect();
        if visible.is_empty() {
            continue;
        }

        // A `BTreeMap`, not a `HashMap`: the horizon is read back by iterating
        // this, and `HashMap`'s order is seeded randomly per process -- which
        // made the same scene hull to the same solid with its triangles in a
        // different order on every run, so no two exports of a hull were ever
        // byte-identical (spec section 5.2: evaluation is deterministic).
        let mut edge_count: BTreeMap<(usize, usize), i32> = BTreeMap::new();
        for &fi in &visible {
            let v = faces[fi].v;
            for k in 0..3 {
                let a = v[k];
                let b = v[(k + 1) % 3];
                *edge_count.entry((a, b)).or_insert(0) += 1;
            }
        }
        let horizon: Vec<(usize, usize)> =
            edge_count.keys().filter(|&&(a, b)| !edge_count.contains_key(&(b, a))).cloned().collect();

        let visible_set: std::collections::HashSet<usize> = visible.into_iter().collect();
        let mut new_faces: Vec<Face> = faces
            .iter()
            .enumerate()
            .filter(|(idx, _)| !visible_set.contains(idx))
            .map(|(_, f)| Face { v: f.v, normal: f.normal })
            .collect();

        for (a, b) in horizon {
            add_face(&pts, [a, b, i], &mut new_faces);
        }
        faces = new_faces;
    }

    let mut mesh = Mesh::new();
    for f in &faces {
        mesh.push_triangle(pts[f.v[0]], pts[f.v[1]], pts[f.v[2]]);
    }
    mesh
}
