//! The shapes that come to a point.

use super::super::*;
use simple3d_geom::primitives as gen;

pub(super) const CONE: PrimitiveSpec = PrimitiveSpec {
    type_id: "cone",
    label: "Cone",
    category: CONES,
    params: &[
        ParamSpec::positive_length("bottom_diameter", "Bottom diameter", 20.0),
        ParamSpec::length("top_diameter", "Top diameter (0 = point)", 0.0),
        ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ParamSpec::sweep(),
    ],
    segmented: true,
    build: |p, seg| {
        gen::cone_sector_mesh(p.num("bottom_diameter"), p.num("top_diameter"), p.num("height"), seg, p.num("sweep"))
    },
    axes: |p| {
        // Whichever end is wider sets the X/Y extent.
        let wider = if p.num("top_diameter") > p.num("bottom_diameter") { "top_diameter" } else { "bottom_diameter" };
        let [x, y] = round_axes(p, wider, wider);
        [x, y, Some(AxisDriver::direct("height"))]
    },
};

pub(super) const PYRAMID: PrimitiveSpec = PrimitiveSpec {
    type_id: "pyramid",
    label: "Pyramid",
    category: CONES,
    params: &[
        ParamSpec::positive_length("base_width", "Base width (X)", 20.0),
        ParamSpec::positive_length("base_depth", "Base depth (Y)", 20.0),
        ParamSpec::length("top_width", "Top width (0 = apex)", 0.0),
        ParamSpec::length("top_depth", "Top depth (0 = apex)", 0.0),
        ParamSpec::positive_length("height", "Height (Z)", 20.0),
    ],
    segmented: false,
    build: |p, _seg| {
        gen::pyramid_mesh(
            p.num("base_width"),
            p.num("base_depth"),
            p.num("top_width"),
            p.num("top_depth"),
            p.num("height"),
        )
    },
    axes: |p| {
        let wx = if p.num("top_width") > p.num("base_width") { "top_width" } else { "base_width" };
        let wy = if p.num("top_depth") > p.num("base_depth") { "top_depth" } else { "base_depth" };
        [Some(AxisDriver::direct(wx)), Some(AxisDriver::direct(wy)), Some(AxisDriver::direct("height"))]
    },
};

pub(super) const REGULAR_PYRAMID: PrimitiveSpec = PrimitiveSpec {
    type_id: "regular_pyramid",
    label: "Regular pyramid",
    category: CONES,
    params: &[
        ParamSpec::sides(6),
        ParamSpec::positive_length("base_diameter", "Base diameter", 20.0),
        ParamSpec::length("top_diameter", "Top diameter (0 = apex)", 0.0),
        ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ParamSpec::choice("measure", "Diameter measured", MEASURE),
    ],
    segmented: false,
    build: |p, _seg| {
        gen::regular_pyramid_mesh(
            p.int("sides"),
            p.num("base_diameter"),
            p.num("top_diameter"),
            p.num("height"),
            p.int("measure") == 1,
        )
    },
    axes: |p| {
        let wider = if p.num("top_diameter") > p.num("base_diameter") { "top_diameter" } else { "base_diameter" };
        [Some(AxisDriver { param: wider, factor: polygon_x_factor(p) }), None, Some(AxisDriver::direct("height"))]
    },
};
