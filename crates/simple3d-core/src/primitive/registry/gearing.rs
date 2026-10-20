//! The gears that mesh with the spur gear (issue 123): an internal gear for planetary gearboxes.

use super::super::*;
use super::mechanics::gear_spec;
use simple3d_geom::primitives as gen;

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
