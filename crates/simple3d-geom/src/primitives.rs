//! One generator function per primitive type from the spec (section 3.2).
//! Every shape is generated centred on its own local origin (the default
//! "centre" anchor); `Mesh::apply_base_anchor` converts to the "base" anchor
//! afterward. Every shape's stated dimensions are exact outer bounding
//! dimensions in its own frame, before rotation, per the spec's precision
//! requirement -- on flat axes exactly, on curved axes to within tessellation
//! (vertices lie exactly on the circumscribed circle/sphere).

mod boxes;
pub(crate) use boxes::*;
pub use boxes::{
    box_mesh, chamfer_limit, chamfered_box_mesh, corner_box_mesh, plate_mesh, rounded_box_mesh, rounded_plate_mesh,
    slot_mesh, wedge_mesh, ChamferEdges,
};
mod round;
pub use round::{
    cone_mesh, cone_sector_mesh, cylinder_mesh, cylinder_sector_mesh, disc_mesh, disc_sector_mesh, pyramid_mesh,
    regular_prism_mesh, regular_pyramid_mesh, ring_mesh, ring_sector_mesh, torus_mesh, tube_mesh, tube_sector_mesh,
};
mod sphere;
pub use sphere::{capsule_mesh, ellipsoid_mesh, spherical_cap_mesh};
mod platonic;
pub use platonic::{dodecahedron_mesh, icosahedron_mesh, octahedron_mesh, tetrahedron_mesh};
