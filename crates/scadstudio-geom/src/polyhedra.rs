//! The four regular polyhedra. Each is built as a "unit" construction
//! centred on the origin, then uniformly scaled so its measured circumsphere
//! diameter or edge length matches what the user asked for -- computed
//! numerically (rather than from memorised closed-form ratios) to avoid
//! transcription errors.

use crate::mesh::Mesh;
use crate::vec3::Vec3;

pub enum SizeMode {
    CircumscribedDiameter,
    EdgeLength,
}

fn build_polyhedron(vertices: &[Vec3], faces: &[[usize; 3]]) -> Mesh {
    let mut mesh = Mesh::new();
    for f in faces {
        let (a, b, c) = (vertices[f[0]], vertices[f[1]], vertices[f[2]]);
        let n = (b - a).cross(c - a);
        let centroid = (a + b + c) * (1.0 / 3.0);
        if n.dot(centroid) < 0.0 {
            mesh.push_triangle(a, c, b);
        } else {
            mesh.push_triangle(a, b, c);
        }
    }
    mesh
}

fn scale_for(vertices: &[Vec3], size: f64, mode: &SizeMode) -> f64 {
    match mode {
        SizeMode::CircumscribedDiameter => {
            let r = vertices.iter().map(|v| v.length()).fold(0.0, f64::max);
            (size / 2.0) / r
        }
        SizeMode::EdgeLength => {
            let mut min_d = f64::MAX;
            for i in 0..vertices.len() {
                for j in i + 1..vertices.len() {
                    min_d = min_d.min((vertices[i] - vertices[j]).length());
                }
            }
            size / min_d
        }
    }
}

pub fn tetrahedron_mesh(size: f64, mode: SizeMode) -> Mesh {
    let v = vec![
        Vec3::new(1.0, 1.0, 1.0),
        Vec3::new(1.0, -1.0, -1.0),
        Vec3::new(-1.0, 1.0, -1.0),
        Vec3::new(-1.0, -1.0, 1.0),
    ];
    let s = scale_for(&v, size, &mode);
    let v: Vec<Vec3> = v.iter().map(|p| *p * s).collect();
    let faces = [[1, 2, 3], [0, 3, 2], [0, 1, 3], [0, 2, 1]];
    build_polyhedron(&v, &faces)
}

pub fn octahedron_mesh(size: f64, mode: SizeMode) -> Mesh {
    let v = vec![
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.0, 0.0, -1.0),
    ];
    let s = scale_for(&v, size, &mode);
    let v: Vec<Vec3> = v.iter().map(|p| *p * s).collect();
    let faces = [
        [0, 2, 4], [0, 2, 5], [0, 3, 4], [0, 3, 5],
        [1, 2, 4], [1, 2, 5], [1, 3, 4], [1, 3, 5],
    ];
    build_polyhedron(&v, &faces)
}

fn icosahedron_unit() -> (Vec<Vec3>, Vec<[usize; 3]>) {
    let t = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let v = vec![
        Vec3::new(-1.0, t, 0.0),
        Vec3::new(1.0, t, 0.0),
        Vec3::new(-1.0, -t, 0.0),
        Vec3::new(1.0, -t, 0.0),
        Vec3::new(0.0, -1.0, t),
        Vec3::new(0.0, 1.0, t),
        Vec3::new(0.0, -1.0, -t),
        Vec3::new(0.0, 1.0, -t),
        Vec3::new(t, 0.0, -1.0),
        Vec3::new(t, 0.0, 1.0),
        Vec3::new(-t, 0.0, -1.0),
        Vec3::new(-t, 0.0, 1.0),
    ];
    let faces: Vec<[usize; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    (v, faces)
}

pub fn icosahedron_mesh(size: f64, mode: SizeMode) -> Mesh {
    let (v, faces) = icosahedron_unit();
    let s = scale_for(&v, size, &mode);
    let v: Vec<Vec3> = v.iter().map(|p| *p * s).collect();
    build_polyhedron(&v, &faces)
}

/// Built as the dual of the icosahedron: a dodecahedron vertex sits at each
/// icosahedron face centroid, and a dodecahedron (pentagonal) face surrounds
/// each icosahedron vertex, fan-triangulated into 3 triangles.
pub fn dodecahedron_mesh(size: f64, mode: SizeMode) -> Mesh {
    let (ico_v, ico_f) = icosahedron_unit();
    let dode_v: Vec<Vec3> = ico_f
        .iter()
        .map(|f| ((ico_v[f[0]] + ico_v[f[1]] + ico_v[f[2]]) * (1.0 / 3.0)).normalized())
        .collect();

    let mut faces = Vec::new();
    for (vi, v) in ico_v.iter().enumerate() {
        let incident: Vec<usize> =
            ico_f.iter().enumerate().filter(|(_, f)| f.contains(&vi)).map(|(i, _)| i).collect();
        let normal = v.normalized();
        let tangent = if normal.x.abs() < 0.9 { Vec3::new(1.0, 0.0, 0.0) } else { Vec3::new(0.0, 1.0, 0.0) };
        let u = normal.cross(tangent).normalized();
        let w = normal.cross(u);
        let mut pts: Vec<(usize, f64)> = incident
            .iter()
            .map(|&fi| {
                let d = dode_v[fi];
                (fi, d.dot(u).atan2(d.dot(w)))
            })
            .collect();
        pts.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        for k in 1..pts.len() - 1 {
            faces.push([pts[0].0, pts[k].0, pts[k + 1].0]);
        }
    }

    let s = scale_for(&dode_v, size, &mode);
    let dode_v: Vec<Vec3> = dode_v.iter().map(|p| *p * s).collect();
    build_polyhedron(&dode_v, &faces)
}
