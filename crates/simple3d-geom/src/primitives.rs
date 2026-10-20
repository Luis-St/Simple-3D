//! One generator per primitive type (spec section 3.2), centred on its origin (`apply_base_anchor` makes
//! the base anchor). Stated dimensions are exact outer bounds before rotation: flat axes exactly, curved
//! axes to tessellation, with vertices on the circumscribed circle or sphere.

mod boxes;
pub(crate) use boxes::*;
pub use boxes::{
    box_mesh, chamfered_box_cut_mesh, chamfered_box_mesh, corner_box_mesh, plate_corners_mesh, plate_mesh,
    rounded_box_mesh, rounded_plate_mesh, slot_mesh, wedge_mesh, wedge_shifted_mesh, ChamferEdges,
};
mod rounded;
pub use rounded::{rounded_box_edges_mesh, slot_shaped_mesh, RoundEdges};
mod round;
pub use round::{
    cone_mesh, cone_sector_mesh, cylinder_mesh, cylinder_sector_mesh, disc_mesh, disc_sector_mesh,
    hollow_cone_sector_mesh, pyramid_mesh, regular_prism_mesh, regular_pyramid_mesh, ring_mesh, ring_sector_mesh,
    skewed, tapered_cylinder_sector_mesh, tapered_tube_sector_mesh, torus_mesh, tube_mesh, tube_sector_mesh,
    twisted_prism_mesh,
};
mod sphere;
pub use sphere::{
    cap_shell_mesh, cap_sphere_diameter, capsule_mesh, ellipsoid_mesh, ellipsoid_shell_mesh, spherical_cap_mesh,
};
mod platonic;
pub use platonic::{dodecahedron_mesh, icosahedron_mesh, octahedron_mesh, tetrahedron_mesh};
mod fastener;
pub use fastener::{hex_nut_mesh, screw_hole_mesh, screw_hole_shaped_mesh, ScrewHead};
mod gear;
pub use gear::{
    bevel_gear_mesh, gear_mesh, internal_gear_mesh, rack_mesh, spur_gear_mesh, worm_mesh, BevelSpec, Body, GearSpec,
    RackSpec, ToothProfile,
};
mod spring;
pub use spring::{coil_spring_mesh, spring_mesh, spring_pitch_for_height, SpringEnds};
mod thread;
pub use thread::{thread_mesh, threaded_rod_mesh, ThreadProfile};
