//! The shapes printed parts are fastened with (issue 123): nuts, standoffs and screw holes; the moving
//! parts are in `mechanics`. Their defaults are metric hardware sizes, to set to the part at hand.

use super::super::*;
use simple3d_geom::primitives::{self as gen, ScrewHead};

/// Across corners over across flats, for a hexagon with a corner on X.
const HEX_CORNERS: f64 = 1.154_700_538_379_251_5;

const fn hex_params(hole: f64, height: f64) -> [ParamSpec; 4] {
    [
        // The key stays from when the size was always across the flats, so older files keep theirs.
        ParamSpec::positive_length("across_flats", "Hex size", 5.5),
        ParamSpec::choice("measure", "Hex size measured", HEX_MEASURE),
        ParamSpec::length("hole_diameter", "Hole diameter (0 = solid)", hole),
        ParamSpec::positive_length("height", "Height (Z)", height),
    ]
}

fn hex_build(p: &Params, seg: u32) -> simple3d_geom::Mesh {
    let flats = p.num("across_flats") / hex_factor(p);
    gen::hex_nut_mesh(flats, p.num("hole_diameter"), p.num("height"), seg)
}

/// The hex size over the width across its flats: 1, or across corners over across flats.
fn hex_factor(p: &Params) -> f64 {
    if p.int("measure") == 1 {
        HEX_CORNERS
    } else {
        1.0
    }
}

fn hex_axes(p: &Params) -> [Option<AxisDriver>; 3] {
    let factor = hex_factor(p);
    [
        Some(AxisDriver { param: "across_flats", factor: HEX_CORNERS / factor }),
        Some(AxisDriver { param: "across_flats", factor: 1.0 / factor }),
        Some(AxisDriver::direct("height")),
    ]
}

pub(super) const HEX_NUT: PrimitiveSpec = PrimitiveSpec {
    type_id: "hex_nut",
    label: "Hex nut",
    category: PRINTING,
    params: &hex_params(3.0, 2.4),
    segmented: true,
    build: hex_build,
    axes: hex_axes,
};

pub(super) const HEX_STANDOFF: PrimitiveSpec = PrimitiveSpec {
    type_id: "hex_standoff",
    label: "Hex standoff",
    category: PRINTING,
    params: &hex_params(2.5, 10.0),
    segmented: true,
    build: hex_build,
    axes: hex_axes,
};

pub(super) const SCREW_HOLE: PrimitiveSpec = PrimitiveSpec {
    type_id: "screw_hole",
    label: "Screw hole",
    category: PRINTING,
    params: &[
        ParamSpec::positive_length("shaft_diameter", "Shaft diameter", 3.4),
        ParamSpec::choice("head", "Head", SCREW_HEAD),
        ParamSpec::positive_length("head_diameter", "Head diameter", 6.5).when("head", 0),
        ParamSpec::angle("head_angle", "Countersink angle", 90.0, 1.0, 179.0).when("head", 0),
        ParamSpec::positive_length("bore_diameter", "Counterbore diameter", 6.0).when("head", 1),
        ParamSpec::length("recess", "Head recess", 0.0).when("head", 0),
        ParamSpec::positive_length("bore_depth", "Counterbore depth", 3.0).when("head", 1),
        ParamSpec::positive_length("length", "Length (Z)", 12.0),
        ParamSpec::choice("head_side", "Head at", HEAD_SIDE),
    ],
    segmented: true,
    build: |p, seg| {
        let (head, wide) = screw_head(p);
        let recess = if p.int("head") == 0 { p.num("recess") } else { 0.0 };
        let (shaft, length, bottom) = (p.num("shaft_diameter"), p.num("length"), p.int("head_side") == 1);
        gen::screw_hole_shaped_mesh(shaft, p.num(wide), length, head, recess, bottom, seg)
    },
    axes: |p| {
        // The head is the widest part, unless it is given narrower than the shaft or there is none.
        let wide = screw_head(p).1;
        let across = if p.num(wide) > p.num("shaft_diameter") { wide } else { "shaft_diameter" };
        let d = Some(AxisDriver::direct(across));
        [d, d, Some(AxisDriver::direct("length"))]
    },
};

/// The head a screw hole's choice asks for, with the parameter giving its diameter.
fn screw_head(p: &Params) -> (ScrewHead, &'static str) {
    match p.int("head") {
        0 => (ScrewHead::Countersunk { angle: p.num("head_angle") }, "head_diameter"),
        1 => (ScrewHead::Counterbored { depth: p.num("bore_depth") }, "bore_diameter"),
        _ => (ScrewHead::Plain, "shaft_diameter"),
    }
}
