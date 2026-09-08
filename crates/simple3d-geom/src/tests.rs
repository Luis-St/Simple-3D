#![cfg(test)]

mod boolean_basic;
mod boolean_contact;
mod boolean_disjoint;
mod boolean_robust;
mod boolean_shape;
mod dimensions;
mod planar;
mod primitives;

use crate::vec3::Vec3;

use crate::mesh::Mesh;

fn assert_manifold(name: &str, mesh: &Mesh) {
    assert!(mesh.triangle_count() > 0, "{name}: empty mesh");
    if let Some(issue) = mesh.manifold_issue() {
        panic!("{name}: not manifold: {issue}");
    }
}

fn assert_bounds(name: &str, mesh: &Mesh, expect: Vec3, tol: f64) {
    let (lo, hi) = mesh.bounds().unwrap();
    let size = hi - lo;
    assert!(
        (size.x - expect.x).abs() <= tol && (size.y - expect.y).abs() <= tol && (size.z - expect.z).abs() <= tol,
        "{name}: bounds {:?} vs expected {:?}",
        size,
        expect
    );
}

/// A rolling landscape tile: a grid of heights turned into a closed solid with
/// a skirt and a floor, a hundred samples across.
///
/// Built here rather than taken from anywhere, because what the test below
/// needs is simply a *dense, many-planed* solid -- eighteen thousand faces in
/// as many directions -- which is the shape of operand the kernel used to fall
/// apart on.
fn landscape(nx: usize, ny: usize) -> Mesh {
    let (width, depth) = (200.0, 200.0);
    let at = |i: usize, j: usize| {
        let (x, y) = (i as f64 / (nx - 1) as f64 * width, j as f64 / (ny - 1) as f64 * depth);
        let z = 9.0 + 6.0 * (x / 47.0).sin() * (y / 39.0).cos() + 2.0 * (x / 13.0).cos();
        Vec3::new(x, y, z)
    };
    let floor = |i: usize, j: usize| {
        let p = at(i, j);
        Vec3::new(p.x, p.y, 0.0)
    };
    let mut mesh = Mesh::new();
    let mut quad = |a: Vec3, b: Vec3, c: Vec3, d: Vec3| {
        mesh.push_triangle(a, b, c);
        mesh.push_triangle(a, c, d);
    };
    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            quad(at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
            quad(floor(i, j), floor(i, j + 1), floor(i + 1, j + 1), floor(i + 1, j));
        }
    }
    // The four skirts, each walking one edge of the grid down to the floor.
    for i in 0..nx - 1 {
        quad(at(i, 0), floor(i, 0), floor(i + 1, 0), at(i + 1, 0));
        quad(at(i + 1, ny - 1), floor(i + 1, ny - 1), floor(i, ny - 1), at(i, ny - 1));
    }
    for j in 0..ny - 1 {
        quad(at(0, j + 1), floor(0, j + 1), floor(0, j), at(0, j));
        quad(at(nx - 1, j), floor(nx - 1, j), floor(nx - 1, j + 1), at(nx - 1, j + 1));
    }
    mesh.weld()
}

/// The volume the mesh encloses, by the divergence theorem. Any boolean that
/// leaves an operand's interior faces in the result, or loses part of the
/// surface, gets this badly wrong -- which a triangle count alone will not show.
fn volume(mesh: &Mesh) -> f64 {
    let mut total = 0.0;
    for t in &mesh.indices {
        let (a, b, c) = (mesh.positions[t[0] as usize], mesh.positions[t[1] as usize], mesh.positions[t[2] as usize]);
        total += a.dot(b.cross(c)) / 6.0;
    }
    total
}
