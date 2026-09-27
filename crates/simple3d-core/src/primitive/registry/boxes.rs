//! Boxes, and the shapes cut or drawn from a box.

use super::super::*;
use simple3d_geom::primitives as gen;

pub(super) const BOX: PrimitiveSpec = PrimitiveSpec {
    type_id: "box",
    label: "Box",
    category: BOXES,
    params: &[
        ParamSpec::positive_length("width", "Width (X)", 20.0),
        ParamSpec::positive_length("depth", "Depth (Y)", 20.0),
        ParamSpec::positive_length("height", "Height (Z)", 20.0),
    ],
    segmented: false,
    build: |p, _seg| gen::box_mesh(p.num("width"), p.num("depth"), p.num("height")),
    axes: |_p| {
        [Some(AxisDriver::direct("width")), Some(AxisDriver::direct("depth")), Some(AxisDriver::direct("height"))]
    },
};

pub(super) const ROUNDED_BOX: PrimitiveSpec = PrimitiveSpec {
    type_id: "rounded_box",
    label: "Rounded box",
    category: BOXES,
    params: &[
        ParamSpec::positive_length("width", "Width (X)", 20.0),
        ParamSpec::positive_length("depth", "Depth (Y)", 20.0),
        ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ParamSpec::choice("corner_style", "Corners", CORNER_STYLE),
        ParamSpec::length("corner_radius", "Corner radius", 3.0),
        ParamSpec::count("corner_segments", "Corner segments", 6, 1, 64).when("corner_style", 0),
    ],
    segmented: false,
    build: |p, _seg| {
        gen::corner_box_mesh(
            p.num("width"),
            p.num("depth"),
            p.num("height"),
            p.num("corner_radius"),
            p.int("corner_segments"),
            p.int("corner_style") == 1,
        )
    },
    axes: |_p| {
        [Some(AxisDriver::direct("width")), Some(AxisDriver::direct("depth")), Some(AxisDriver::direct("height"))]
    },
};

pub(super) const CHAMFERED_BOX: PrimitiveSpec = PrimitiveSpec {
    type_id: "chamfered_box",
    label: "Chamfered box",
    category: BOXES,
    params: &[
        ParamSpec::positive_length("width", "Width (X)", 20.0),
        ParamSpec::positive_length("depth", "Depth (Y)", 20.0),
        ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ParamSpec::length("chamfer", "Chamfer", 2.0),
        ParamSpec::choice("edges", "Edges cut", CHAMFER_EDGES),
    ],
    segmented: false,
    build: |p, _seg| {
        gen::chamfered_box_mesh(
            p.num("width"),
            p.num("depth"),
            p.num("height"),
            p.num("chamfer"),
            gen::ChamferEdges::from_index(p.int("edges")),
        )
    },
    // A chamfer never cuts past a face, so every axis keeps the box's dimension.
    axes: |_p| {
        [Some(AxisDriver::direct("width")), Some(AxisDriver::direct("depth")), Some(AxisDriver::direct("height"))]
    },
};

pub(super) const WEDGE: PrimitiveSpec = PrimitiveSpec {
    type_id: "wedge",
    label: "Wedge",
    category: BOXES,
    params: &[
        ParamSpec::positive_length("width", "Width (X)", 20.0),
        ParamSpec::positive_length("depth", "Depth (Y)", 20.0),
        ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ParamSpec::length("top_width", "Top width (0 = sharp ridge)", 0.0),
    ],
    segmented: false,
    build: |p, _seg| gen::wedge_mesh(p.num("width"), p.num("depth"), p.num("height"), p.num("top_width")),
    axes: |_p| {
        [Some(AxisDriver::direct("width")), Some(AxisDriver::direct("depth")), Some(AxisDriver::direct("height"))]
    },
};

pub(super) const PRISM: PrimitiveSpec = PrimitiveSpec {
    type_id: "prism",
    label: "Regular prism",
    category: BOXES,
    params: &[
        ParamSpec::sides(6),
        ParamSpec::positive_length("diameter", "Diameter", 20.0),
        ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ParamSpec::choice("measure", "Diameter measured", MEASURE),
    ],
    segmented: false,
    build: |p, _seg| gen::regular_prism_mesh(p.int("sides"), p.num("diameter"), p.num("height"), p.int("measure") == 1),
    axes: |p| {
        [
            Some(AxisDriver { param: "diameter", factor: polygon_x_factor(p) }),
            // An n-gon's Y extent is neither diameter for general n, so no handle rather than a misleading one.
            None,
            Some(AxisDriver::direct("height")),
        ]
    },
};
