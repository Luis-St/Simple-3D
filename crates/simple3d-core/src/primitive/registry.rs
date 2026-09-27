//! Every palette shape, in offer order; definitions live in the modules below by kind.

mod boxes;
mod cones;
mod flat;
mod polyhedra;
mod round;

use super::*;

pub static REGISTRY: &[PrimitiveSpec] = &[
    // Boxes and prisms
    boxes::BOX,
    boxes::ROUNDED_BOX,
    boxes::CHAMFERED_BOX,
    boxes::WEDGE,
    boxes::PRISM,
    // Round solids
    round::SPHERE,
    round::SPHERICAL_CAP,
    round::CYLINDER,
    round::TUBE,
    round::CAPSULE,
    round::TORUS,
    // Cones and pyramids
    cones::CONE,
    cones::PYRAMID,
    cones::REGULAR_PYRAMID,
    // Regular polyhedra
    polyhedra::TETRAHEDRON,
    polyhedra::OCTAHEDRON,
    polyhedra::DODECAHEDRON,
    polyhedra::ICOSAHEDRON,
    // Flat shapes
    flat::PLATE,
    flat::DISC,
    flat::RING,
    flat::SLOT,
];
