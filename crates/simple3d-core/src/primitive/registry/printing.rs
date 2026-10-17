//! The shapes printed parts are made of or cut with (issue 123): nuts, standoffs, screw holes,
//! threads, gears and springs. Their defaults are metric hardware sizes, to set to the part at hand.

use super::super::*;
use simple3d_geom::primitives::{self as gen, ScrewHead};

/// Across corners over across flats, for a hexagon with a corner on X.
const HEX_CORNERS: f64 = 1.154_700_538_379_251_5;

const fn hex_params(hole: f64, height: f64) -> [ParamSpec; 3] {
    [
        ParamSpec::positive_length("across_flats", "Across flats (Y)", 5.5),
        ParamSpec::length("hole_diameter", "Hole diameter (0 = solid)", hole),
        ParamSpec::positive_length("height", "Height (Z)", height),
    ]
}

fn hex_build(p: &Params, seg: u32) -> simple3d_geom::Mesh {
    gen::hex_nut_mesh(p.num("across_flats"), p.num("hole_diameter"), p.num("height"), seg)
}

fn hex_axes(_p: &Params) -> [Option<AxisDriver>; 3] {
    [
        Some(AxisDriver { param: "across_flats", factor: HEX_CORNERS }),
        Some(AxisDriver::direct("across_flats")),
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
        ParamSpec::positive_length("bore_depth", "Counterbore depth", 3.0).when("head", 1),
        ParamSpec::positive_length("length", "Length (Z)", 12.0),
    ],
    segmented: true,
    build: |p, seg| {
        let (head, wide) = screw_head(p);
        gen::screw_hole_mesh(p.num("shaft_diameter"), p.num(wide), p.num("length"), head, seg)
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

pub(super) const THREADED_ROD: PrimitiveSpec = PrimitiveSpec {
    type_id: "threaded_rod",
    label: "Threaded rod",
    category: PRINTING,
    params: &[
        ParamSpec::positive_length("diameter", "Thread diameter", 8.0),
        ParamSpec::positive_length("pitch", "Pitch", 1.25),
        ParamSpec::positive_length("length", "Length (Z)", 20.0),
    ],
    segmented: true,
    build: |p, seg| gen::threaded_rod_mesh(p.num("diameter"), p.num("pitch"), p.num("length"), seg),
    // The crests wind past the axes between the rows they are drawn with, so only Z has a handle.
    axes: |_p| [None, None, Some(AxisDriver::direct("length"))],
};

pub(super) const SPUR_GEAR: PrimitiveSpec = PrimitiveSpec {
    type_id: "spur_gear",
    label: "Spur gear",
    category: PRINTING,
    params: &[
        ParamSpec::count("teeth", "Teeth", 20, 8, 400),
        ParamSpec::positive_length("module", "Module (pitch diameter / teeth)", 1.0),
        ParamSpec::angle("pressure_angle", "Pressure angle", 20.0, 10.0, 30.0),
        ParamSpec::positive_length("thickness", "Thickness (Z)", 5.0),
        ParamSpec::length("bore", "Bore diameter (0 = solid)", 5.0),
    ],
    segmented: false,
    build: |p, _seg| {
        gen::spur_gear_mesh(p.int("teeth"), p.num("module"), p.num("pressure_angle"), p.num("thickness"), p.num("bore"))
    },
    axes: |p| {
        // Across the tips, where a tooth points both ways along the axis.
        let teeth = p.int("teeth").max(8);
        let tips = Some(AxisDriver { param: "module", factor: teeth as f64 + 2.0 });
        [tips.filter(|_| teeth % 2 == 0), tips.filter(|_| teeth % 4 == 0), Some(AxisDriver::direct("thickness"))]
    },
};

pub(super) const SPRING: PrimitiveSpec = PrimitiveSpec {
    type_id: "spring",
    label: "Spring",
    category: PRINTING,
    params: &[
        ParamSpec::positive_length("diameter", "Outer diameter", 12.0),
        ParamSpec::positive_length("wire", "Wire diameter", 1.5),
        ParamSpec::positive_length("pitch", "Pitch", 4.0),
        ParamSpec::count("turns", "Turns", 6, 1, 200),
    ],
    segmented: true,
    build: |p, seg| gen::spring_mesh(p.num("diameter"), p.num("wire"), p.num("pitch"), p.int("turns") as f64, seg),
    // Its height is pitch, turns and wire together, so only X and Y have a handle.
    axes: |_p| [Some(AxisDriver::direct("diameter")), Some(AxisDriver::direct("diameter")), None],
};
