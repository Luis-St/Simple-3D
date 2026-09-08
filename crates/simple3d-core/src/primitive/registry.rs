//! Every shape the palette offers, as one table.
//!
//! The definitions themselves are grouped by kind in the modules below;
//! this is the order they are offered in.

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
