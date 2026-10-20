//! The gears that mesh with the spur gear (issue 123) are solids of the size they are given.

use super::*;
use crate::primitives::*;
use std::f64::consts::PI;

#[test]
pub(crate) fn an_internal_gear_has_its_teeth_inside_a_rim_of_its_diameter() {
    for profile in [ToothProfile::Involute, ToothProfile::Cycloidal, ToothProfile::CircularArc] {
        let mut spec = GearSpec::new(30, 1.0, 20.0, 5.0, 0.0);
        spec.profile = profile;
        let ring = internal_gear_mesh(&spec, 40.0);
        assert_manifold("internal gear", &ring);
        assert_bounds("internal gear", &ring, Vec3::new(40.0, 40.0, 5.0), 1e-9);
        // The teeth end a module inside the pitch circle.
        let nearest = ring.positions.iter().map(|p| p.x.hypot(p.y)).fold(f64::MAX, f64::min);
        assert!((nearest - 14.0).abs() < 1e-9 && ring.signed_volume() > 0.0, "{profile:?}: teeth reach {nearest}");
    }
}

#[test]
pub(crate) fn a_rack_is_as_long_as_its_teeth_and_as_high_as_it_is_given() {
    for profile in [ToothProfile::Involute, ToothProfile::Cycloidal, ToothProfile::CircularArc] {
        let mut spec =
            RackSpec { teeth: 10, module: 1.0, profile, thickness: 5.0, height: 8.0, helix: 0.0, herringbone: false };
        let rack = rack_mesh(&spec);
        assert_manifold("rack", &rack);
        assert_bounds("rack", &rack, Vec3::new(10.0 * PI, 8.0, 5.0), 1e-9);
        assert!(rack.signed_volume() > 0.0, "{profile:?} is inside out");
        spec.helix = 20.0;
        assert_manifold("helical rack", &rack_mesh(&spec));
        spec.herringbone = true;
        assert_manifold("herringbone rack", &rack_mesh(&spec));
    }
}

#[test]
pub(crate) fn a_bevel_gear_is_as_wide_as_its_tips_at_the_back() {
    for profile in [ToothProfile::Involute, ToothProfile::Cycloidal, ToothProfile::CircularArc] {
        for mate_teeth in [20, 40] {
            let spec = BevelSpec { teeth: 20, mate_teeth, module: 1.0, profile, face_width: 4.0, bore: 4.0 };
            let gear = bevel_gear_mesh(&spec);
            assert_manifold("bevel gear", &gear);
            let (lo, hi) = gear.bounds().unwrap();
            let wide = 2.0 * spec.tip_radius();
            assert!(
                (hi.x - lo.x - wide).abs() < 1e-9 && (hi.y - lo.y - wide).abs() < 1e-9,
                "{profile:?}: {lo:?} to {hi:?}"
            );
            assert!(
                (hi.z + lo.z).abs() < 1e-9 && gear.signed_volume() > 0.0,
                "{profile:?} is off centre or inside out"
            );
        }
    }
}

#[test]
pub(crate) fn a_worm_reaches_a_module_past_its_pitch_diameter() {
    for starts in [1, 3] {
        let worm = worm_mesh(1.0, starts, 10.0, 20.0, false, 48);
        assert_manifold("worm", &worm);
        let (lo, hi) = worm.bounds().unwrap();
        assert!((hi.z - lo.z - 20.0).abs() < 1e-9 && worm.signed_volume() > 0.0);
        // Off the axis, where the end caps are fanned from.
        let radii: Vec<f64> = worm.positions.iter().map(|p| p.x.hypot(p.y)).filter(|&r| r > 1e-9).collect();
        let (inner, outer) = radii.iter().fold((f64::MAX, 0.0f64), |(a, b), &r| (a.min(r), b.max(r)));
        assert!((outer - 6.0).abs() < 1e-9 && (inner - 3.75).abs() < 1e-9, "{starts} starts: {inner} to {outer}");
    }
}
