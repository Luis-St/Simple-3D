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
        ParamSpec::choice("round_edges", "Rounded edges", ROUND_EDGES).when("corner_style", 0),
        ParamSpec::length("corner_radius", "Corner radius", 3.0),
        ParamSpec::count("corner_segments", "Corner segments", 6, 1, 64).when("corner_style", 0),
    ],
    segmented: false,
    build: |p, _seg| {
        let (w, d, h, size) = (p.num("width"), p.num("depth"), p.num("height"), p.num("corner_radius"));
        if p.int("corner_style") == 1 {
            return gen::corner_box_mesh(w, d, h, size, 0, true);
        }
        let edges = gen::RoundEdges::from_index(p.int("round_edges"));
        gen::rounded_box_edges_mesh(w, d, h, size, p.int("corner_segments"), edges)
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
        ParamSpec::choice("chamfer_mode", "Chamfer given", CHAMFER_MODE),
        ParamSpec::length("chamfer", "Chamfer", 2.0).when("chamfer_mode", 0),
        ParamSpec::length("chamfer_x", "Chamfer along X", 2.0).when("chamfer_mode", 1),
        ParamSpec::length("chamfer_y", "Chamfer along Y", 2.0).when("chamfer_mode", 1),
        ParamSpec::length("chamfer_z", "Chamfer along Z", 2.0).when("chamfer_mode", 1),
        ParamSpec::choice("edges", "Edges cut", CHAMFER_EDGES),
    ],
    segmented: false,
    build: |p, _seg| {
        let cut = match p.int("chamfer_mode") {
            1 => [p.num("chamfer_x"), p.num("chamfer_y"), p.num("chamfer_z")],
            _ => [p.num("chamfer"); 3],
        };
        let edges = gen::ChamferEdges::from_index(p.int("edges"));
        gen::chamfered_box_cut_mesh(p.num("width"), p.num("depth"), p.num("height"), cut, edges)
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
        ParamSpec::length("top_width", "Top size (0 = sharp ridge)", 0.0),
        ParamSpec::choice("top_axis", "Narrows along", AXIS_XY),
        ParamSpec::choice("top_align", "Top placed", TOP_PLACED),
        ParamSpec::signed_length("top_offset", "Top offset", 0.0).when("top_align", 3),
    ],
    segmented: false,
    build: |p, _seg| {
        let along_y = p.int("top_axis") == 1;
        gen::wedge_shifted_mesh(
            p.num("width"),
            p.num("depth"),
            p.num("height"),
            p.num("top_width"),
            wedge_offset(p),
            along_y,
        )
    },
    axes: |p| {
        // The axis it narrows along spans the base unless the top reaches past it.
        let (narrowed, full) = if p.int("top_axis") == 1 { (1, "depth") } else { (0, "width") };
        let (base, top, offset) = (p.num(full), p.num("top_width"), wedge_offset(p));
        let driver = if offset.abs() + top / 2.0 <= base / 2.0 + 1e-12 {
            Some(AxisDriver::direct(full))
        } else if offset == 0.0 {
            Some(AxisDriver::direct("top_width"))
        } else {
            None
        };
        let mut axes =
            [Some(AxisDriver::direct("width")), Some(AxisDriver::direct("depth")), Some(AxisDriver::direct("height"))];
        axes[narrowed] = driver;
        axes
    },
};

/// How far a wedge's top is moved off the middle, along the axis it narrows along.
fn wedge_offset(p: &Params) -> f64 {
    let full = if p.int("top_axis") == 1 { p.num("depth") } else { p.num("width") };
    let spare = (full - p.num("top_width")) / 2.0;
    match p.int("top_align") {
        1 => -spare,
        2 => spare,
        3 => p.num("top_offset"),
        _ => 0.0,
    }
}

pub(super) const PRISM: PrimitiveSpec = PrimitiveSpec {
    type_id: "prism",
    label: "Regular prism",
    category: BOXES,
    params: &[
        ParamSpec::sides(6),
        ParamSpec::positive_length("diameter", "Diameter", 20.0),
        ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ParamSpec::choice("measure", "Diameter measured", MEASURE),
        ParamSpec::angle("twist", "Twist", 0.0, -360.0, 360.0),
        ParamSpec::length("wall", "Wall thickness (0 = solid)", 0.0),
    ],
    segmented: false,
    build: |p, _seg| {
        gen::twisted_prism_mesh(
            p.int("sides"),
            p.num("diameter"),
            p.num("height"),
            p.int("measure") == 1,
            p.num("twist"),
            p.num("wall"),
        )
    },
    axes: |p| {
        [
            // Twisted, its corners come round to where its flats were.
            (p.num("twist") == 0.0).then(|| AxisDriver { param: "diameter", factor: polygon_x_factor(p) }),
            // An n-gon's Y extent is neither diameter for general n, so no handle rather than a misleading one.
            None,
            Some(AxisDriver::direct("height")),
        ]
    },
};
