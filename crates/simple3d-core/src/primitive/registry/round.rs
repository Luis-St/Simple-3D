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
        ParamSpec::choice("part", "Part", SPHERE_PART),
        ParamSpec::length("wall", "Wall thickness (0 = solid)", 0.0),
    ],
    segmented: true,
    build: |p, seg| {
        let (dx, dy, dz) = (p.num("diameter_x"), p.num("diameter_y"), p.num("diameter_z"));
        gen::ellipsoid_shell_mesh(dx, dy, dz, p.num("wall"), p.int("part") == 1, seg)
    },
    axes: |p| {
        // The upper half is half as tall.
        let z = if p.int("part") == 1 { 0.5 } else { 1.0 };
        [
            Some(AxisDriver::direct("diameter_x")),
            Some(AxisDriver::direct("diameter_y")),
            Some(AxisDriver { param: "diameter_z", factor: z }),
        ]
    },
};

pub(super) const SPHERICAL_CAP: PrimitiveSpec = PrimitiveSpec {
    type_id: "spherical_cap",
    label: "Spherical cap",
    category: ROUND,
    params: &[
        ParamSpec::choice("size_mode", "Size given as", CAP_SIZE),
        ParamSpec::positive_length("diameter", "Diameter", 20.0).when("size_mode", 0),
        ParamSpec::positive_length("base_diameter", "Base diameter", 20.0).when("size_mode", 1),
        ParamSpec::positive_length("cap_height", "Cap height", 10.0),
        ParamSpec::length("wall", "Wall thickness (0 = solid)", 0.0),
    ],
    segmented: true,
    build: |p, seg| {
        let (height, wall) = (p.num("cap_height"), p.num("wall"));
        match p.int("size_mode") {
            // A base and a height always make a cap: of the sphere through the rim and the top.
            1 => gen::cap_shell_mesh(gen::cap_sphere_diameter(p.num("base_diameter"), height), height, wall, seg),
            _ if wall > 0.0 => gen::cap_shell_mesh(p.num("diameter"), height, wall, seg),
            _ => gen::spherical_cap_mesh(p.num("diameter"), height, seg),
        }
    },
    axes: |p| {
        let cap = p.num("cap_height");
        if p.int("size_mode") == 1 {
            // Up to a half sphere the rim is the widest part; past it the sphere's middle is.
            let d = (cap <= p.num("base_diameter") / 2.0).then(|| AxisDriver::direct("base_diameter"));
            return [d, d, Some(AxisDriver::direct("cap_height"))];
        }
        // A shallow cap's rim diameter depends on both parameters, so no X/Y handle.
        let diameter = p.num("diameter");
        let d = (cap >= diameter / 2.0).then(|| AxisDriver::direct("diameter"));
        // The generator holds the cap height to the diameter, beyond which Z would stop tracking.
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
        ParamSpec::count("top_size", "Top size (%)", 100, 0, 1000),
        ParamSpec::sweep(),
    ],
    segmented: true,
    build: |p, seg| {
        let (dx, dy, top) = (p.num("diameter_x"), p.num("diameter_y"), p.num("top_size") / 100.0);
        gen::tapered_cylinder_sector_mesh(dx, dy, p.num("height"), top, seg, p.num("sweep"))
    },
    axes: |p| {
        // A flared top is the wider end.
        let factor = (p.num("top_size") / 100.0).max(1.0);
        let [x, y] = round_axes(p, "diameter_x", "diameter_y").map(|d| d.map(|d| AxisDriver { factor, ..d }));
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
        ParamSpec::choice("top", "Top", TUBE_TOP),
        ParamSpec::positive_length("top_outer_diameter", "Top outer diameter", 20.0).when("top", 1),
        ParamSpec::length("top_inner_diameter", "Top inner diameter", 16.0).when("top", 1),
        ParamSpec::sweep(),
    ],
    segmented: true,
    build: |p, seg| {
        let (outer, inner, height, sweep) = (p.num("outer_diameter"), tube_inner(p), p.num("height"), p.num("sweep"));
        if p.int("top") == 0 {
            return gen::tube_sector_mesh(outer, inner, height, seg, sweep);
        }
        let top = [p.num("top_outer_diameter"), p.num("top_inner_diameter")];
        gen::tapered_tube_sector_mesh([outer, top[0]], [inner, top[1]], height, seg, sweep)
    },
    axes: |p| {
        let flared = p.int("top") == 1 && p.num("top_outer_diameter") > p.num("outer_diameter");
        let wider = if flared { "top_outer_diameter" } else { "outer_diameter" };
        let [x, y] = round_axes(p, wider, wider);
        [x, y, Some(AxisDriver::direct("height"))]
    },
};

pub(super) const CAPSULE: PrimitiveSpec = PrimitiveSpec {
    type_id: "capsule",
    label: "Capsule",
    category: ROUND,
    params: &[
        ParamSpec::positive_length("diameter", "Diameter", 10.0),
        ParamSpec::choice("length_mode", "Length measured", CAPSULE_LENGTH),
        ParamSpec::positive_length("length", "Total length (with caps)", 30.0).when("length_mode", 0),
        ParamSpec::length("straight_length", "Straight length", 20.0).when("length_mode", 1),
    ],
    segmented: true,
    build: |p, seg| gen::capsule_mesh(p.num("diameter"), capsule_length(p), seg),
    axes: |p| {
        // A capsule shorter than its diameter is a sphere; the diameter governs Z then. Measured by
        // its straight part, its length is that and the diameter together, which no handle drives.
        let z = match p.int("length_mode") {
            1 => None,
            _ if p.num("length") >= p.num("diameter") => Some(AxisDriver::direct("length")),
            _ => Some(AxisDriver::direct("diameter")),
        };
        [Some(AxisDriver::direct("diameter")), Some(AxisDriver::direct("diameter")), z]
    },
};

/// A capsule's length over its caps, however it is measured.
fn capsule_length(p: &Params) -> f64 {
    match p.int("length_mode") {
        1 => p.num("straight_length") + p.num("diameter"),
        _ => p.num("length"),
    }
}

pub(super) const TORUS: PrimitiveSpec = PrimitiveSpec {
    type_id: "torus",
    label: "Torus",
    category: ROUND,
    params: &[
        ParamSpec::choice("size_mode", "Size given as", TORUS_SIZE),
        ParamSpec::positive_length("ring_diameter", "Ring diameter (centre-line)", 30.0).when("size_mode", 0),
        ParamSpec::positive_length("tube_diameter", "Tube diameter", 6.0).when("size_mode", 0),
        ParamSpec::positive_length("outer_diameter", "Outer diameter", 36.0).when("size_mode", 1),
        ParamSpec::length("inner_diameter", "Inner diameter", 24.0).when("size_mode", 1),
        ParamSpec::sweep(),
    ],
    segmented: true,
    build: |p, seg| {
        let (ring, tube) = torus_size(p);
        gen::torus_mesh(ring, tube, p.num("sweep"), seg)
    },
    axes: |p| {
        if p.int("size_mode") == 1 {
            // The outside is the outer diameter however wide the tube; its height is the tube's.
            let [x, y] = round_axes(p, "outer_diameter", "outer_diameter");
            return [x, y, None];
        }
        // X and Y are ring plus tube diameter together, so only Z has a handle.
        [None, None, Some(AxisDriver::direct("tube_diameter"))]
    },
};

/// A torus's centre-line and tube diameters, however it is sized. An inner diameter reaching the
/// outer one leaves the thinnest of tubes rather than none.
fn torus_size(p: &Params) -> (f64, f64) {
    if p.int("size_mode") == 0 {
        return (p.num("ring_diameter"), p.num("tube_diameter"));
    }
    let outer = p.num("outer_diameter");
    let inner = p.num("inner_diameter").clamp(0.0, outer * 0.999);
    ((outer + inner) / 2.0, (outer - inner) / 2.0)
}
