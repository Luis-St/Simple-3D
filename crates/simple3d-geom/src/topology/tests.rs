use super::*;
use crate::primitives::{box_mesh, cylinder_mesh, ellipsoid_mesh};
use crate::vec3::Vec3;

fn cube() -> Mesh {
    box_mesh(10.0, 10.0, 10.0).weld()
}

/// Two cubes sharing the edge along x = 5, y = 5, so four triangles meet there.
fn cubes_on_an_edge() -> Mesh {
    let mut mesh = cube();
    mesh.append(&cube().translated(Vec3::new(10.0, 10.0, 0.0)));
    mesh.weld()
}

#[test]
fn a_cube_has_eighteen_edges_each_between_two_triangles() {
    let mesh = cube();
    let edges = EdgeTable::of(&mesh);
    assert_eq!((mesh.positions.len(), mesh.indices.len()), (8, 12));
    assert_eq!(edges.len(), 18);
    for (ends, faces) in edges.iter() {
        assert!(ends[0] < ends[1]);
        assert_eq!(faces.len(), 2, "edge {ends:?}");
        assert!(faces[0] < faces[1]);
        for &face in faces {
            assert!(mesh.indices[face as usize].contains(&ends[0]) && mesh.indices[face as usize].contains(&ends[1]));
        }
    }
}

#[test]
fn every_edge_is_found_from_either_end_and_a_missing_one_is_not() {
    let mesh = cube();
    let edges = EdgeTable::of(&mesh);
    for e in 0..edges.len() {
        let [a, b] = edges.ends(e);
        assert_eq!(edges.find(a, b), Some(e));
        assert_eq!(edges.find(b, a), Some(e));
    }
    // 8 vertices, 18 edges out of 28 pairs: some pair has none.
    let missing = (0..8u32).flat_map(|a| (a + 1..8).map(move |b| (a, b))).filter(|&(a, b)| edges.find(a, b).is_none());
    assert_eq!(missing.count(), 28 - 18);
    assert_eq!(edges.find(0, 99), None);
    assert_eq!(edges.find(99, 100), None);
}

#[test]
fn neighbours_are_sorted_symmetric_and_match_the_edges() {
    let mesh = ellipsoid_mesh(20.0, 20.0, 20.0, 16).weld();
    let topology = Topology::of(&mesh);
    assert_eq!(topology.neighbours.len(), mesh.positions.len());
    assert_eq!(topology.neighbours.items().len(), topology.edges.len() * 2);
    for v in 0..mesh.positions.len() {
        let row = topology.neighbours.row(v);
        assert!(row.windows(2).all(|pair| pair[0] < pair[1]), "row {v} is {row:?}");
        for &n in row {
            assert!(topology.neighbours.row(n as usize).contains(&(v as u32)));
            assert!(topology.edges.find(v as u32, n).is_some());
        }
    }
}

#[test]
fn vertex_faces_list_each_triangle_under_its_three_corners() {
    let mesh = cube();
    let faces = vertex_faces(&mesh);
    assert_eq!(faces.items().len(), 36);
    for v in 0..mesh.positions.len() {
        for &face in faces.row(v) {
            assert!(mesh.indices[face as usize].contains(&(v as u32)));
        }
    }
}

#[test]
fn a_closed_surface_has_no_boundary_and_an_open_one_does() {
    assert!(Topology::of(&cube()).boundary_vertices().iter().all(|&rim| !rim));
    let mut open = cube();
    open.indices.pop();
    let rim = Topology::of(&open).boundary_vertices();
    assert_eq!(rim.iter().filter(|&&on| on).count(), 3);
    let edges = EdgeTable::of(&open);
    assert_eq!(edges.iter().filter(|(_, faces)| faces.len() == 1).count(), 3);
}

#[test]
fn touching_bodies_meet_in_a_junction_and_stay_one_body() {
    let mesh = cubes_on_an_edge();
    let edges = EdgeTable::of(&mesh);
    assert_eq!(edges.iter().filter(|(_, faces)| faces.len() == 4).count(), 1);
    assert!(connected_bodies(&mesh).iter().all(|&body| body == 0));
}

#[test]
fn separate_bodies_are_numbered_in_order_of_their_lowest_vertex() {
    let mut mesh = cube();
    mesh.append(&cube().translated(Vec3::new(30.0, 0.0, 0.0)));
    mesh.append(&cube().translated(Vec3::new(60.0, 0.0, 0.0)));
    let bodies = connected_bodies(&mesh.weld());
    assert_eq!(bodies.len(), 24);
    assert_eq!(bodies, (0..24).map(|v| v / 8).collect::<Vec<u32>>());
}

#[test]
fn a_cube_corner_points_along_its_diagonal_however_its_faces_are_split() {
    let mesh = cube();
    let (lo, hi) = mesh.bounds().expect("not empty");
    let centre = (lo + hi) * 0.5;
    for (p, n) in mesh.positions.iter().zip(vertex_normals(&mesh)) {
        let d = *p - centre;
        let diagonal = Vec3::new(d.x.signum(), d.y.signum(), d.z.signum()).normalized();
        assert!((n - diagonal).length() < 1e-9, "{p:?} has {n:?}");
    }
}

#[test]
fn a_sphere_normal_points_away_from_its_centre() {
    let mesh = ellipsoid_mesh(20.0, 20.0, 20.0, 32).weld();
    for (p, n) in mesh.positions.iter().zip(vertex_normals(&mesh)) {
        assert!((n.length() - 1.0).abs() < 1e-9);
        assert!(n.dot(p.normalized()) > 0.99, "{p:?} has {n:?}");
    }
}

#[test]
fn a_vertex_with_no_triangle_has_no_normal_and_no_neighbours() {
    let mut mesh = cube();
    mesh.positions.push(Vec3::new(50.0, 0.0, 0.0));
    let topology = Topology::of(&mesh);
    assert_eq!(vertex_normals(&mesh)[8], Vec3::ZERO);
    assert!(topology.neighbours.row(8).is_empty());
    assert!(topology.faces.row(8).is_empty());
    assert_eq!(connected_bodies(&mesh)[8], 1);
}

#[test]
fn an_empty_mesh_has_empty_tables() {
    let topology = Topology::of(&Mesh::new());
    assert!(topology.edges.is_empty());
    assert!(topology.neighbours.is_empty());
    assert!(topology.boundary_vertices().is_empty());
}

/// Corner normals of `mesh` at a 30 degree crease, every triangle smooth.
fn smoothed(mesh: &Mesh) -> Vec<[Vec3; 3]> {
    let topology = Topology::of(mesh);
    corner_normals(mesh, &topology.edges, &topology.faces, 30.0, &vec![true; mesh.indices.len()])
}

#[test]
fn a_cube_stays_sharp_with_each_corner_along_its_own_face() {
    let mesh = cube();
    for (tri, corners) in mesh.indices.iter().zip(smoothed(&mesh)) {
        let face = mesh.triangle_normal(*tri);
        for n in corners {
            assert!((n - face).length() < 1e-9, "{n:?} on a face facing {face:?}");
        }
    }
}

#[test]
fn a_cylinder_side_shades_round_and_its_caps_stay_flat() {
    let mesh = cylinder_mesh(20.0, 20.0, 10.0, 32).weld();
    let (lo, hi) = mesh.bounds().expect("not empty");
    let axis = (lo + hi) * 0.5;
    for (tri, corners) in mesh.indices.iter().zip(smoothed(&mesh)) {
        let face = mesh.triangle_normal(*tri);
        for (k, n) in corners.into_iter().enumerate() {
            if face.z.abs() > 0.99 {
                assert!((n - face).length() < 1e-9, "a cap corner leans: {n:?}");
            } else {
                let p = mesh.positions[tri[k] as usize];
                let radial = Vec3::new(p.x - axis.x, p.y - axis.y, 0.0).normalized();
                assert!((n - radial).length() < 1e-9, "a side corner at {p:?} has {n:?}");
            }
        }
    }
}

#[test]
fn a_sphere_corner_matches_its_vertex_normal() {
    let mesh = ellipsoid_mesh(20.0, 20.0, 20.0, 32).weld();
    let vertex = vertex_normals(&mesh);
    for (tri, corners) in mesh.indices.iter().zip(smoothed(&mesh)) {
        for k in 0..3 {
            assert!((corners[k] - vertex[tri[k] as usize]).length() < 1e-9);
        }
    }
}

#[test]
fn a_triangle_left_out_has_no_corners_and_cuts_the_smoothing_off() {
    let mesh = ellipsoid_mesh(20.0, 20.0, 20.0, 32).weld();
    let topology = Topology::of(&mesh);
    let mut smooth = vec![true; mesh.indices.len()];
    smooth[0] = false;
    let corners = corner_normals(&mesh, &topology.edges, &topology.faces, 30.0, &smooth);
    assert_eq!(corners[0], [Vec3::ZERO; 3]);
    // Its corners' other triangles no longer average it in.
    let vertex = vertex_normals(&mesh);
    let shared = mesh.indices[0][0];
    let (face, at) = (1..mesh.indices.len())
        .find_map(|f| mesh.indices[f].iter().position(|&c| c == shared).map(|k| (f, k)))
        .expect("another triangle at the corner");
    assert!(corners[face][at].length() > 0.999);
    assert!((corners[face][at] - vertex[shared as usize]).length() > 1e-6);
}

/// Moving a few vertices and updating only them and their neighbours (the brush, issue 127) gives
/// what computing every normal again gives.
#[test]
fn normals_updated_around_moved_vertices_match_a_full_recomputation() {
    let mut mesh = ellipsoid_mesh(10.0, 10.0, 10.0, 24).weld();
    let topology = Topology::of(&mesh);
    let smooth = vec![true; mesh.indices.len()];
    let mut vertex = vertex_normals(&mesh);
    let mut corners = corner_normals(&mesh, &topology.edges, &topology.faces, 30.0, &smooth);
    // A sharp dent, so some corners split at the new creases.
    let moved: Vec<u32> = vec![40, 41, 65];
    for &v in &moved {
        mesh.positions[v as usize] = mesh.positions[v as usize] * 0.6;
    }
    let mut ring: Vec<u32> = moved.iter().flat_map(|&v| topology.neighbours.row(v as usize).to_vec()).collect();
    ring.extend(&moved);
    ring.sort_unstable();
    ring.dedup();
    vertex_normals_at(&mesh, &topology.faces, &ring, &mut vertex);
    corner_normals_at(&mesh, &topology.edges, &topology.faces, 30.0, &smooth, &ring, &mut corners);
    let close = |a: Vec3, b: Vec3| (a - b).length() < 1e-12;
    assert!(vertex.iter().zip(vertex_normals(&mesh)).all(|(&a, b)| close(a, b)));
    let full = corner_normals(&mesh, &topology.edges, &topology.faces, 30.0, &smooth);
    assert!(corners.iter().zip(&full).all(|(a, b)| (0..3).all(|k| close(a[k], b[k]))));
}
