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
        ParamSpec::length("wall", "Wall thickness (0 = solid)", 0.0),
        ParamSpec::sweep(),
    ],
    segmented: true,
    build: |p, seg| {
        let (bottom, top, height) = (p.num("bottom_diameter"), p.num("top_diameter"), p.num("height"));
        gen::hollow_cone_sector_mesh(bottom, top, height, p.num("wall"), seg, p.num("sweep"))
    },
    axes: |p| {
        // The wider end sets the X/Y extent.
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
        ParamSpec::signed_length("top_offset_x", "Top offset X", 0.0),
        ParamSpec::signed_length("top_offset_y", "Top offset Y", 0.0),
    ],
    segmented: false,
    build: |p, _seg| {
        let mesh = gen::pyramid_mesh(
            p.num("base_width"),
            p.num("base_depth"),
            p.num("top_width"),
            p.num("top_depth"),
            p.num("height"),
        );
        gen::skewed(mesh, p.num("height"), p.num("top_offset_x"), p.num("top_offset_y"))
    },
    axes: |p| {
        let x = leaning_axis(p, "base_width", "top_width", "top_offset_x");
        let y = leaning_axis(p, "base_depth", "top_depth", "top_offset_y");
        [x, y, Some(AxisDriver::direct("height"))]
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
        ParamSpec::signed_length("top_offset_x", "Top offset X", 0.0),
        ParamSpec::signed_length("top_offset_y", "Top offset Y", 0.0),
    ],
    segmented: false,
    build: |p, _seg| {
        let mesh = gen::regular_pyramid_mesh(
            p.int("sides"),
            p.num("base_diameter"),
            p.num("top_diameter"),
            p.num("height"),
            p.int("measure") == 1,
        );
        gen::skewed(mesh, p.num("height"), p.num("top_offset_x"), p.num("top_offset_y"))
    },
    axes: |p| {
        // Leaning, its X extent is base and top side by side, which no one parameter drives.
        let upright = p.num("top_offset_x") == 0.0 && p.num("top_offset_y") == 0.0;
        let wider = if p.num("top_diameter") > p.num("base_diameter") { "top_diameter" } else { "base_diameter" };
        let x = upright.then(|| AxisDriver { param: wider, factor: polygon_x_factor(p) });
        [x, None, Some(AxisDriver::direct("height"))]
    },
};

/// A pyramid's extent along one axis: the wider of base and top while upright, and the base while a
/// leaning top stays within it. A top leaning out past the base is both together, with no handle.
fn leaning_axis(p: &Params, base: &'static str, top: &'static str, offset: &'static str) -> Option<AxisDriver> {
    let (b, t, o) = (p.num(base), p.num(top), p.num(offset));
    if o == 0.0 {
        Some(AxisDriver::direct(if t > b { top } else { base }))
    } else if o.abs() + t / 2.0 <= b / 2.0 {
        Some(AxisDriver::direct(base))
    } else {
        None
    }
}
