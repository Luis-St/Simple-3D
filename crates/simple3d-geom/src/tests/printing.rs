//! The shapes for printed parts (issue 123) are solids of the size they are given.

use super::*;
use crate::primitives::*;
use std::f64::consts::PI;

#[test]
pub(crate) fn a_hex_nut_is_a_hexagon_with_a_hole_through_it() {
    let nut = hex_nut_mesh(5.5, 3.0, 2.4, 32);
    assert_manifold("hex nut", &nut);
    assert_bounds("hex nut", &nut, Vec3::new(5.5 / (PI / 6.0).cos(), 5.5, 2.4), 1e-9);
    let hexagon = 3.0_f64.sqrt() / 2.0 * 5.5 * 5.5;
    let hole = PI * 1.5 * 1.5;
    let volume = nut.signed_volume();
    assert!(volume > 0.0 && (volume - (hexagon - hole) * 2.4).abs() < 0.05 * volume, "volume {volume}");
    let solid = hex_nut_mesh(5.5, 0.0, 2.4, 32);
    assert!((solid.signed_volume() - hexagon * 2.4).abs() < 1e-9, "a hole of 0 is not solid");
}

#[test]
pub(crate) fn every_screw_head_widens_to_its_diameter_at_the_top() {
    for head in [ScrewHead::Countersunk { angle: 90.0 }, ScrewHead::Counterbored { depth: 3.0 }] {
        let hole = screw_hole_mesh(3.4, 6.5, 12.0, head, 32);
        assert_manifold("screw hole", &hole);
        assert_bounds("screw hole", &hole, Vec3::new(6.5, 6.5, 12.0), 1e-9);
        let top = hole.positions.iter().filter(|p| p.z > 6.0 - 1e-9).map(|p| p.x.hypot(p.y)).fold(0.0, f64::max);
        let bottom = hole.positions.iter().filter(|p| p.z < -6.0 + 1e-9).map(|p| p.x.hypot(p.y)).fold(0.0, f64::max);
        assert!((top - 3.25).abs() < 1e-9 && (bottom - 1.7).abs() < 1e-9, "{head:?}: top {top}, bottom {bottom}");
    }
    let plain = screw_hole_mesh(3.4, 6.5, 12.0, ScrewHead::Plain, 32);
    assert_bounds("plain screw hole", &plain, Vec3::new(3.4, 3.4, 12.0), 1e-9);
}

#[test]
pub(crate) fn a_spur_gear_reaches_its_tip_circle_and_keeps_its_bore() {
    let gear = spur_gear_mesh(20, 1.0, 20.0, 5.0, 5.0);
    assert_manifold("spur gear", &gear);
    assert_bounds("spur gear", &gear, Vec3::new(22.0, 22.0, 5.0), 1e-9);
    let nearest = gear.positions.iter().map(|p| p.x.hypot(p.y)).fold(f64::MAX, f64::min);
    assert!((nearest - 2.5).abs() < 1e-9, "the bore is {nearest} in radius");
    assert!(gear.signed_volume() > 0.0);
    assert_manifold("solid gear", &spur_gear_mesh(9, 2.0, 30.0, 5.0, 0.0));
}

#[test]
pub(crate) fn every_tooth_profile_reaches_its_own_tip_circle() {
    // The circular arc's teeth end in a half circle on the pitch circle, short of one module.
    let tips = [
        (ToothProfile::Involute, 11.0),
        (ToothProfile::Cycloidal, 11.0),
        (ToothProfile::CircularArc, 10.0 + 10.0 * (PI / 40.0).tan()),
    ];
    for (profile, tip) in tips {
        let mut spec = GearSpec::new(20, 1.0, 20.0, 5.0, 5.0);
        spec.profile = profile;
        let gear = gear_mesh(&spec);
        assert_manifold("gear", &gear);
        let farthest = gear.positions.iter().map(|p| p.x.hypot(p.y)).fold(0.0, f64::max);
        assert!((farthest - tip).abs() < 1e-9 && gear.signed_volume() > 0.0, "{profile:?}: tip {farthest}");
    }
}

#[test]
pub(crate) fn a_keyway_leaves_nothing_inside_the_bore() {
    // A keyway corner once landed on a tooth's radial root edge and was put at the middle, a sliver
    // fanned across the face.
    let mut spec = GearSpec::new(20, 1.0, 20.0, 5.0, 5.0);
    spec.key = (2.0, 1.0);
    let gear = gear_mesh(&spec);
    assert_manifold("keyed gear", &gear);
    let nearest = gear.positions.iter().map(|p| p.x.hypot(p.y)).fold(f64::MAX, f64::min);
    assert!((nearest - 2.5).abs() < 1e-9, "a point {nearest} from the middle");
}

#[test]
pub(crate) fn a_threaded_rod_winds_between_its_crests_and_roots() {
    let rod = threaded_rod_mesh(8.0, 1.25, 20.0, 32);
    assert_manifold("threaded rod", &rod);
    assert!(rod.signed_volume() > 0.0);
    let radii: Vec<f64> = rod.positions.iter().map(|p| p.x.hypot(p.y)).filter(|r| *r > 1e-9).collect();
    let (least, most) = radii.iter().fold((f64::MAX, 0.0_f64), |(lo, hi), &r| (lo.min(r), hi.max(r)));
    let minor = 4.0 - 0.625 * 0.866_025 * 1.25;
    assert!((most - 4.0).abs() < 1e-9 && (least - minor).abs() < 1e-9, "radii {least}..{most}");
    let (lo, hi) = rod.bounds().unwrap();
    assert!((hi.z - lo.z - 20.0).abs() < 1e-9);
}

#[test]
pub(crate) fn a_spring_is_as_wide_and_tall_as_its_coils() {
    let spring = spring_mesh(12.0, 1.5, 4.0, 6.0, 32);
    assert_manifold("spring", &spring);
    assert!(spring.signed_volume() > 0.0);
    assert_bounds("spring", &spring, Vec3::new(12.0, 12.0, 6.0 * 4.0 + 1.5), 1e-9);
}
