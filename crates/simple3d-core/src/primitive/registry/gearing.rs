//! The gears that mesh with the spur gear (issue 123): an internal gear for planetary gearboxes and a
//! rack for a straight push.

use super::super::*;
use super::mechanics::gear_spec;
use simple3d_geom::primitives::{self as gen, RackSpec, ToothProfile};
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
