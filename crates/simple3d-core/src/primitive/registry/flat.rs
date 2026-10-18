//! The shapes that are wide and thin.

use super::super::*;
use simple3d_geom::primitives as gen;

pub(super) const PLATE: PrimitiveSpec = PrimitiveSpec {
    type_id: "plate",
    label: "Plate",
    category: FLAT,
    params: &[
        ParamSpec::positive_length("width", "Width (X)", 40.0),
        ParamSpec::positive_length("depth", "Depth (Y)", 20.0),
        ParamSpec::positive_length("thickness", "Thickness (Z)", 4.0),
        ParamSpec::choice("corner_mode", "Corners", PLATE_CORNERS),
        ParamSpec::length("corner_radius", "Corner radius (0 = square)", 0.0).when("corner_mode", 0),
        ParamSpec::length("radius_front_right", "Front right radius (+X -Y)", 0.0).when("corner_mode", 1),
        ParamSpec::length("radius_back_right", "Back right radius (+X +Y)", 0.0).when("corner_mode", 1),
        ParamSpec::length("radius_back_left", "Back left radius (-X +Y)", 0.0).when("corner_mode", 1),
        ParamSpec::length("radius_front_left", "Front left radius (-X -Y)", 0.0).when("corner_mode", 1),
        ParamSpec::count("corner_segments", "Corner segments", 6, 1, 64),
    ],
    segmented: false,
    build: |p, _seg| {
        let (w, d, t, seg) = (p.num("width"), p.num("depth"), p.num("thickness"), p.int("corner_segments"));
        if p.int("corner_mode") == 1 {
            let keys = ["radius_front_right", "radius_back_right", "radius_back_left", "radius_front_left"];
            return gen::plate_corners_mesh(w, d, t, keys.map(|key| p.num(key)), seg);
        }
        gen::rounded_plate_mesh(w, d, t, p.num("corner_radius"), seg)
    },
    axes: |_p| {
        [Some(AxisDriver::direct("width")), Some(AxisDriver::direct("depth")), Some(AxisDriver::direct("thickness"))]
    },
};

pub(super) const DISC: PrimitiveSpec = PrimitiveSpec {
    type_id: "disc",
    label: "Disc",
    category: FLAT,
    params: &[
        ParamSpec::locked_length("diameter_x", "Diameter X", 20.0, 1),
        ParamSpec::locked_length("diameter_y", "Diameter Y", 20.0, 1),
        ParamSpec::positive_length("thickness", "Thickness (Z)", 2.0),
        ParamSpec::sweep(),
    ],
    segmented: true,
    build: |p, seg| {
        gen::disc_sector_mesh(p.num("diameter_x"), p.num("diameter_y"), p.num("thickness"), seg, p.num("sweep"))
    },
    axes: |p| {
        let [x, y] = round_axes(p, "diameter_x", "diameter_y");
        [x, y, Some(AxisDriver::direct("thickness"))]
    },
};

pub(super) const RING: PrimitiveSpec = PrimitiveSpec {
    type_id: "ring",
    label: "Ring",
    category: FLAT,
    params: &[
        ParamSpec::positive_length("outer_diameter", "Outer diameter", 20.0),
        ParamSpec::choice("wall_mode", "Wall given as", WALL_MODE),
        ParamSpec::positive_length("wall_thickness", "Wall thickness", 2.0).when("wall_mode", 0),
        ParamSpec::positive_length("inner_diameter", "Inner diameter", 16.0).when("wall_mode", 1),
        ParamSpec::positive_length("thickness", "Thickness (Z)", 2.0),
        ParamSpec::sweep(),
    ],
    segmented: true,
    build: |p, seg| {
        gen::ring_sector_mesh(p.num("outer_diameter"), tube_inner(p), p.num("thickness"), seg, p.num("sweep"))
    },
    axes: |p| {
        let [x, y] = round_axes(p, "outer_diameter", "outer_diameter");
        [x, y, Some(AxisDriver::direct("thickness"))]
    },
};

pub(super) const SLOT: PrimitiveSpec = PrimitiveSpec {
    type_id: "slot",
    label: "Slot",
    category: FLAT,
    params: &[
        ParamSpec::positive_length("length", "Length (X)", 30.0),
        ParamSpec::positive_length("width", "Width (Y)", 10.0),
        ParamSpec::positive_length("thickness", "Thickness (Z)", 4.0),
        ParamSpec::length("edge_rounding", "Edge rounding (0 = sharp)", 0.0),
        ParamSpec::length("wall", "Wall thickness (0 = solid)", 0.0),
    ],
    segmented: true,
    build: |p, seg| {
        let (length, width, thickness) = (p.num("length"), p.num("width"), p.num("thickness"));
        gen::slot_shaped_mesh(length, width, thickness, p.num("edge_rounding"), p.num("wall"), seg)
    },
    axes: |_p| {
        [Some(AxisDriver::direct("length")), Some(AxisDriver::direct("width")), Some(AxisDriver::direct("thickness"))]
    },
};
