//! The solids of revolution.

use super::super::*;
use simple3d_geom::primitives as gen;

pub(super) const SPHERE: PrimitiveSpec = PrimitiveSpec {
    type_id: "sphere",
    label: "Sphere",
    category: ROUND,
    params: &[
        ParamSpec::locked_length("diameter_x", "Diameter X", 20.0, 1),
        ParamSpec::locked_length("diameter_y", "Diameter Y", 20.0, 1),
        ParamSpec::locked_length("diameter_z", "Diameter Z", 20.0, 1),
    ],
    segmented: true,
    build: |p, seg| gen::ellipsoid_mesh(p.num("diameter_x"), p.num("diameter_y"), p.num("diameter_z"), seg),
    axes: |_p| {
        [
            Some(AxisDriver::direct("diameter_x")),
            Some(AxisDriver::direct("diameter_y")),
            Some(AxisDriver::direct("diameter_z")),
        ]
    },
};

pub(super) const SPHERICAL_CAP: PrimitiveSpec = PrimitiveSpec {
    type_id: "spherical_cap",
    label: "Spherical cap",
    category: ROUND,
    params: &[
        ParamSpec::positive_length("diameter", "Diameter", 20.0),
        ParamSpec::positive_length("cap_height", "Cap height", 10.0),
    ],
    segmented: true,
    build: |p, seg| gen::spherical_cap_mesh(p.num("diameter"), p.num("cap_height"), seg),
    axes: |p| {
        // A cap shallower than a hemisphere is widest at its rim, and the
        // rim diameter depends on both parameters at once -- no single one
        // governs the X/Y extent, so no handle there.
        let (diameter, cap) = (p.num("diameter"), p.num("cap_height"));
        let d = (cap >= diameter / 2.0).then(|| AxisDriver::direct("diameter"));
        // The generator clamps the cap height to the sphere's diameter, so
        // beyond that the Z handle would stop tracking too.
        let z = (cap <= diameter).then(|| AxisDriver::direct("cap_height"));
        [d, d, z]
    },
};

pub(super) const CYLINDER: PrimitiveSpec = PrimitiveSpec {
    type_id: "cylinder",
    label: "Cylinder",
    category: ROUND,
    params: &[
        ParamSpec::locked_length("diameter_x", "Diameter X", 20.0, 1),
        ParamSpec::locked_length("diameter_y", "Diameter Y", 20.0, 1),
        ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ParamSpec::sweep(),
    ],
    segmented: true,
    build: |p, seg| {
        gen::cylinder_sector_mesh(p.num("diameter_x"), p.num("diameter_y"), p.num("height"), seg, p.num("sweep"))
    },
    axes: |p| {
        let [x, y] = round_axes(p, "diameter_x", "diameter_y");
        [x, y, Some(AxisDriver::direct("height"))]
    },
};

pub(super) const TUBE: PrimitiveSpec = PrimitiveSpec {
    type_id: "tube",
    label: "Tube",
    category: ROUND,
    params: &[
        ParamSpec::positive_length("outer_diameter", "Outer diameter", 20.0),
        ParamSpec::choice("wall_mode", "Wall given as", WALL_MODE),
        ParamSpec::positive_length("wall_thickness", "Wall thickness", 2.0).when("wall_mode", 0),
        ParamSpec::positive_length("inner_diameter", "Inner diameter", 16.0).when("wall_mode", 1),
        ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ParamSpec::sweep(),
    ],
    segmented: true,
    build: |p, seg| gen::tube_sector_mesh(p.num("outer_diameter"), tube_inner(p), p.num("height"), seg, p.num("sweep")),
    axes: |p| {
        let [x, y] = round_axes(p, "outer_diameter", "outer_diameter");
        [x, y, Some(AxisDriver::direct("height"))]
    },
};

pub(super) const CAPSULE: PrimitiveSpec = PrimitiveSpec {
    type_id: "capsule",
    label: "Capsule",
    category: ROUND,
    params: &[
        ParamSpec::positive_length("diameter", "Diameter", 10.0),
        ParamSpec::positive_length("length", "Total length (with caps)", 30.0),
    ],
    segmented: true,
    build: |p, seg| gen::capsule_mesh(p.num("diameter"), p.num("length"), seg),
    axes: |p| {
        // A capsule shorter than its own diameter is a sphere: the length
        // no longer governs the Z extent, the diameter does.
        let long_enough = p.num("length") >= p.num("diameter");
        [
            Some(AxisDriver::direct("diameter")),
            Some(AxisDriver::direct("diameter")),
            Some(if long_enough { AxisDriver::direct("length") } else { AxisDriver::direct("diameter") }),
        ]
    },
};

pub(super) const TORUS: PrimitiveSpec = PrimitiveSpec {
    type_id: "torus",
    label: "Torus",
    category: ROUND,
    params: &[
        ParamSpec::positive_length("ring_diameter", "Ring diameter (centre-line)", 30.0),
        ParamSpec::positive_length("tube_diameter", "Tube diameter", 6.0),
        ParamSpec::sweep(),
    ],
    segmented: true,
    build: |p, seg| gen::torus_mesh(p.num("ring_diameter"), p.num("tube_diameter"), p.num("sweep"), seg),
    // The X and Y extents are ring + tube diameter together, so neither
    // parameter alone governs them; only the Z handle is offered.
    axes: |_p| [None, None, Some(AxisDriver::direct("tube_diameter"))],
};
