//! The regular polyhedra, at the radius each is asked for.

use crate::mesh::Mesh;
use crate::polyhedra::{self, SizeMode};

pub fn tetrahedron_mesh(size: f64, as_edge_length: bool) -> Mesh {
    let mode = if as_edge_length { SizeMode::EdgeLength } else { SizeMode::CircumscribedDiameter };
    polyhedra::tetrahedron_mesh(size, mode)
}

pub fn octahedron_mesh(size: f64, as_edge_length: bool) -> Mesh {
    let mode = if as_edge_length { SizeMode::EdgeLength } else { SizeMode::CircumscribedDiameter };
    polyhedra::octahedron_mesh(size, mode)
}

pub fn dodecahedron_mesh(size: f64, as_edge_length: bool) -> Mesh {
    let mode = if as_edge_length { SizeMode::EdgeLength } else { SizeMode::CircumscribedDiameter };
    polyhedra::dodecahedron_mesh(size, mode)
}

pub fn icosahedron_mesh(size: f64, as_edge_length: bool) -> Mesh {
    let mode = if as_edge_length { SizeMode::EdgeLength } else { SizeMode::CircumscribedDiameter };
    polyhedra::icosahedron_mesh(size, mode)
}
