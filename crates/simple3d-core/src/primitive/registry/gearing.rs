//! The gears that mesh with the spur gear (issue 123): an internal gear for planetary gearboxes, a rack
//! for a straight push, a bevel gear for shafts at a right angle and a worm for a large reduction; and
//! a sprocket for roller chain.

use super::super::*;
use super::mechanics::gear_spec;
use simple3d_geom::primitives::{self as gen, BevelSpec, Body, RackSpec, SprocketSpec, ToothProfile};
use std::f64::consts::PI;

pub(super) const INTERNAL_GEAR: PrimitiveSpec = PrimitiveSpec {
    type_id: "internal_gear",
    label: "Internal gear",
    category: PRINTING,
    params: &[
        ParamSpec::count("teeth", "Teeth", 40, 3, 400),
        ParamSpec::positive_length("module", "Module (pitch diameter / teeth)", 1.0),
        ParamSpec::positive_length("thickness", "Thickness (Z)", 5.0),
        ParamSpec::positive_length("outer_diameter", "Outer diameter", 50.0),
        ParamSpec::choice("teeth_form", "Teeth", GEAR_TEETH),
        ParamSpec::angle("helix_angle", "Helix angle", 20.0, -60.0, 60.0).when("teeth_form", 1),
        ParamSpec::flag("herringbone", "Herringbone", false).when("teeth_form", 1),
        ParamSpec::choice("profile", "Tooth profile", GEAR_PROFILE),
    ],
    segmented: false,
    build: |p, _seg| gen::internal_gear_mesh(&gear_spec(p), p.num("outer_diameter")),
    axes: |p| {
        // The rim, while it is not widened to leave a module outside the gaps.
        let fits = p.num("outer_diameter") / 2.0 >= gear_spec(p).internal_rim_radius();
        let rim = fits.then_some(AxisDriver::direct("outer_diameter"));
        [rim, rim, Some(AxisDriver::direct("thickness"))]
    },
};

pub(super) const RACK: PrimitiveSpec = PrimitiveSpec {
    type_id: "gear_rack",
    label: "Gear rack",
    category: PRINTING,
    params: &[
        ParamSpec::count("teeth", "Teeth", 20, 1, 400),
        ParamSpec::positive_length("module", "Module (pitch / pi)", 1.0),
        ParamSpec::positive_length("thickness", "Thickness (Z)", 5.0),
        ParamSpec::positive_length("height", "Height (Y)", 8.0),
        ParamSpec::choice("teeth_form", "Teeth", GEAR_TEETH),
        ParamSpec::angle("helix_angle", "Helix angle", 20.0, -60.0, 60.0).when("teeth_form", 1),
        ParamSpec::flag("herringbone", "Herringbone", false).when("teeth_form", 1),
        ParamSpec::choice("profile", "Tooth profile", GEAR_PROFILE),
    ],
    segmented: false,
    build: |p, _seg| gen::rack_mesh(&rack_spec(p)),
    axes: |p| {
        // Its teeth's length while they stand straight, its height while it leaves a back under them.
        let spec = rack_spec(p);
        let length = (spec.helix == 0.0).then_some(AxisDriver { param: "module", factor: PI * spec.teeth as f64 });
        let height = (spec.height_built() == spec.height).then_some(AxisDriver::direct("height"));
        [length, height, Some(AxisDriver::direct("thickness"))]
    },
};

/// The rack the parameters describe; a hidden helix keeps its values but builds nothing.
fn rack_spec(p: &Params) -> RackSpec {
    let helical = p.int("teeth_form") == 1;
    RackSpec {
        teeth: p.int("teeth").max(1),
        module: p.num("module").max(1e-9),
        profile: ToothProfile::from_index(p.int("profile")),
        thickness: p.num("thickness"),
        height: p.num("height"),
        helix: if helical { p.num("helix_angle") } else { 0.0 },
        herringbone: helical && p.flag("herringbone"),
    }
}

pub(super) const BEVEL_GEAR: PrimitiveSpec = PrimitiveSpec {
    type_id: "bevel_gear",
    label: "Bevel gear",
    category: PRINTING,
    params: &[
        ParamSpec::count("teeth", "Teeth", 20, 3, 400),
        ParamSpec::count("mate_teeth", "Teeth of the meshing gear", 20, 3, 400),
        ParamSpec::positive_length("module", "Module at the back", 1.0),
        ParamSpec::positive_length("face_width", "Face width", 4.0),
        ParamSpec::length("bore", "Bore diameter (0 = solid)", 4.0),
        ParamSpec::choice("profile", "Tooth profile", GEAR_PROFILE),
    ],
    segmented: false,
    build: |p, _seg| gen::bevel_gear_mesh(&bevel_spec(p)),
    axes: |p| {
        // Across the tips at the back, where a tooth points both ways along the axis.
        let spec = bevel_spec(p);
        let tips = Some(AxisDriver { param: "module", factor: 2.0 * spec.tip_radius() / spec.module });
        let teeth = spec.teeth.max(3);
        [tips.filter(|_| teeth % 2 == 0), tips.filter(|_| teeth % 4 == 0), None]
    },
};

fn bevel_spec(p: &Params) -> BevelSpec {
    BevelSpec {
        teeth: p.int("teeth"),
        mate_teeth: p.int("mate_teeth"),
        module: p.num("module").max(1e-9),
        profile: ToothProfile::from_index(p.int("profile")),
        face_width: p.num("face_width"),
        bore: p.num("bore"),
    }
}

pub(super) const WORM: PrimitiveSpec = PrimitiveSpec {
    type_id: "worm",
    label: "Worm",
    category: PRINTING,
    // It drives a helical spur gear of its module, helix angle atan(module * starts / pitch diameter).
    params: &[
        ParamSpec::positive_length("module", "Module (pitch / pi)", 1.0),
        ParamSpec::count("starts", "Starts", 1, 1, 8),
        ParamSpec::positive_length("pitch_diameter", "Pitch diameter", 10.0),
        ParamSpec::positive_length("length", "Length (Z)", 20.0),
        ParamSpec::flag("left_hand", "Left-handed", false),
    ],
    segmented: true,
    build: |p, seg| {
        let (module, starts, length) = (p.num("module"), p.int("starts"), p.num("length"));
        gen::worm_mesh(module, starts, p.num("pitch_diameter"), length, p.flag("left_hand"), seg)
    },
    // The thread winds past the axes between the rows it is drawn with, so only Z has a handle.
    axes: |_p| [None, None, Some(AxisDriver::direct("length"))],
};

pub(super) const SPROCKET: PrimitiveSpec = PrimitiveSpec {
    type_id: "sprocket",
    label: "Chain sprocket",
    category: PRINTING,
    // Sized for ISO 05B chain, 8 mm pitch with 5 mm rollers.
    params: &[
        ParamSpec::count("teeth", "Teeth", 18, 6, 200),
        ParamSpec::positive_length("pitch", "Chain pitch", 8.0),
        ParamSpec::positive_length("roller", "Roller diameter", 5.0),
        ParamSpec::positive_length("thickness", "Thickness (Z)", 2.8),
        ParamSpec::length("bore", "Bore diameter (0 = solid)", 5.0),
        ParamSpec::choice("hub", "Hub", NONE_OR_HUB),
        ParamSpec::positive_length("hub_diameter", "Hub diameter", 12.0).when("hub", 1),
        ParamSpec::positive_length("hub_height", "Hub height", 5.0).when("hub", 1),
        ParamSpec::choice("keyway", "Keyway", NONE_OR_KEYWAY),
        ParamSpec::positive_length("key_width", "Keyway width", 2.0).when("keyway", 1),
        ParamSpec::positive_length("key_depth", "Keyway depth", 1.0).when("keyway", 1),
    ],
    segmented: false,
    build: |p, _seg| gen::sprocket_mesh(&sprocket_spec(p)),
    axes: |p| {
        // Across the tips, where a tooth points both ways along the axis.
        let spec = sprocket_spec(p);
        let tips = Some(AxisDriver { param: "pitch", factor: 2.0 * spec.tip_radius() / spec.pitch });
        let teeth = spec.teeth.max(6);
        let z = (spec.body.hub_built(spec.root_radius()).1 == 0.0).then_some(AxisDriver::direct("thickness"));
        [tips.filter(|_| teeth % 2 == 0), tips.filter(|_| teeth % 4 == 0), z]
    },
};

/// The sprocket the parameters describe; a hidden hub or keyway keeps its values but builds nothing.
fn sprocket_spec(p: &Params) -> SprocketSpec {
    let mut body = Body::new(p.num("thickness"), p.num("bore"));
    if p.int("hub") == 1 {
        body.hub = (p.num("hub_diameter"), p.num("hub_height"));
    }
    if p.int("keyway") == 1 {
        body.key = (p.num("key_width"), p.num("key_depth"));
    }
    SprocketSpec { teeth: p.int("teeth"), pitch: p.num("pitch").max(1e-9), roller: p.num("roller"), body }
}
