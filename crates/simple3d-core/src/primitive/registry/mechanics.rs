//! The moving parts printed for a mechanism (issue 123): threaded rods, gears and springs.

use super::super::*;
use simple3d_geom::primitives::{self as gen, GearSpec, SpringEnds, ThreadProfile, ToothProfile};

pub(super) const THREADED_ROD: PrimitiveSpec = PrimitiveSpec {
    type_id: "threaded_rod",
    label: "Threaded rod",
    category: PRINTING,
    params: &[
        ParamSpec::positive_length("diameter", "Thread diameter", 8.0),
        ParamSpec::positive_length("pitch", "Pitch", 1.25),
        ParamSpec::positive_length("length", "Length (Z)", 20.0),
        ParamSpec::choice("form", "Thread form", THREAD_FORM),
        ParamSpec::angle("thread_angle", "Flank angle", 60.0, 0.0, 179.0).when("form", 3),
        ParamSpec::length("thread_depth", "Thread depth", 0.7).when("form", 3),
        ParamSpec::flag("left_hand", "Left-handed", false),
        ParamSpec::choice("use", "Made as", THREAD_USE),
        ParamSpec::length("clearance", "Clearance (all round)", 0.2).when("use", 1),
    ],
    segmented: true,
    build: |p, seg| {
        let pitch = p.num("pitch");
        let profile = match p.int("form") {
            1 => ThreadProfile::trapezoidal(pitch),
            2 => ThreadProfile::square(pitch),
            3 => ThreadProfile::custom(pitch, p.num("thread_angle"), p.num("thread_depth")),
            _ => ThreadProfile::iso(pitch),
        };
        // A nut's thread is cut with the bolt's, grown by the clearance a print needs to turn.
        let grown = if p.int("use") == 1 { 2.0 * p.num("clearance") } else { 0.0 };
        gen::thread_mesh(p.num("diameter") + grown, pitch, p.num("length"), profile, p.flag("left_hand"), seg)
    },
    // The crests wind past the axes between the rows they are drawn with, so only Z has a handle.
    axes: |_p| [None, None, Some(AxisDriver::direct("length"))],
};

pub(super) const SPUR_GEAR: PrimitiveSpec = PrimitiveSpec {
    type_id: "spur_gear",
    label: "Spur gear",
    category: PRINTING,
    // The everyday gear first; each choice below opens the settings that refine it.
    params: &[
        ParamSpec::count("teeth", "Teeth", 20, 3, 400),
        ParamSpec::positive_length("module", "Module (pitch diameter / teeth)", 1.0),
        ParamSpec::positive_length("thickness", "Thickness (Z)", 5.0),
        ParamSpec::length("bore", "Bore diameter (0 = solid)", 5.0),
        ParamSpec::choice("teeth_form", "Teeth", GEAR_TEETH),
        ParamSpec::angle("helix_angle", "Helix angle", 20.0, -60.0, 60.0).when("teeth_form", 1),
        ParamSpec::flag("herringbone", "Herringbone", false).when("teeth_form", 1),
        ParamSpec::choice("profile", "Tooth profile", GEAR_PROFILE),
        ParamSpec::choice("hub", "Hub", NONE_OR_HUB),
        ParamSpec::positive_length("hub_diameter", "Hub diameter", 10.0).when("hub", 1),
        ParamSpec::positive_length("hub_height", "Hub height", 5.0).when("hub", 1),
        ParamSpec::choice("keyway", "Keyway", NONE_OR_KEYWAY),
        ParamSpec::positive_length("key_width", "Keyway width", 2.0).when("keyway", 1),
        ParamSpec::positive_length("key_depth", "Keyway depth", 1.0).when("keyway", 1),
    ],
    segmented: false,
    build: |p, _seg| gen::gear_mesh(&gear_spec(p)),
    axes: |p| {
        // Across the tips, where a tooth points both ways along the axis; every profile's teeth reach a
        // multiple of the module.
        let spec = gear_spec(p);
        let teeth = p.int("teeth").max(3);
        let tips = Some(AxisDriver { param: "module", factor: 2.0 * spec.tip_radius() / spec.module });
        let z = (spec.hub_built().1 == 0.0).then_some(AxisDriver::direct("thickness"));
        [tips.filter(|_| teeth % 2 == 0), tips.filter(|_| teeth % 4 == 0), z]
    },
};

/// The gear the parameters describe. Only the chosen options count: a hidden helix, hub or keyway keeps
/// its values for when it is chosen again, but builds nothing.
pub(super) fn gear_spec(p: &Params) -> GearSpec {
    let module = p.num("module").max(1e-9);
    let mut spec = GearSpec::new(p.int("teeth"), module, 20.0, p.num("thickness"), p.num("bore"));
    if p.int("teeth_form") == 1 {
        spec.helix = p.num("helix_angle");
        spec.herringbone = p.flag("herringbone");
    }
    spec.profile = ToothProfile::from_index(p.int("profile"));
    if p.int("hub") == 1 {
        spec.hub = (p.num("hub_diameter"), p.num("hub_height"));
    }
    if p.int("keyway") == 1 {
        spec.key = (p.num("key_width"), p.num("key_depth"));
    }
    spec
}

pub(super) const SPRING: PrimitiveSpec = PrimitiveSpec {
    type_id: "spring",
    label: "Spring",
    category: PRINTING,
    params: &[
        ParamSpec::positive_length("diameter", "Outer diameter", 12.0),
        ParamSpec::positive_length("wire", "Wire diameter", 1.5),
        ParamSpec::choice("coil_mode", "Coil given by", SPRING_COIL),
        ParamSpec::length("pitch", "Pitch", 4.0).when("coil_mode", 0),
        ParamSpec::positive_length("height", "Overall height (Z)", 25.5).when("coil_mode", 1),
        ParamSpec::count("turns", "Turns", 6, 1, 200),
        ParamSpec::angle("part_turn", "Part turn", 0.0, 0.0, 359.0),
        ParamSpec::choice("ends", "Ends", SPRING_ENDS),
    ],
    segmented: true,
    build: |p, seg| {
        let ends = SpringEnds::from_index(p.int("ends"));
        gen::coil_spring_mesh(p.num("diameter"), p.num("wire"), spring_pitch(p).max(0.0), spring_turns(p), ends, seg)
    },
    axes: |p| {
        // Its height is pitch, turns and wire together, so only a height it is given has a handle, while
        // that height leaves room for the wire.
        let z = (p.int("coil_mode") == 1 && spring_pitch(p) >= 0.0).then(|| AxisDriver::direct("height"));
        [Some(AxisDriver::direct("diameter")), Some(AxisDriver::direct("diameter")), z]
    },
};

/// The active turns, the part turn included.
fn spring_turns(p: &Params) -> f64 {
    p.num("turns") + p.num("part_turn") / 360.0
}

/// The pitch, given or worked out from the height; negative when the height is too short for the wire.
fn spring_pitch(p: &Params) -> f64 {
    if p.int("coil_mode") == 0 {
        return p.num("pitch");
    }
    let ends = SpringEnds::from_index(p.int("ends"));
    gen::spring_pitch_for_height(p.num("height"), p.num("diameter"), p.num("wire"), spring_turns(p), ends)
}
