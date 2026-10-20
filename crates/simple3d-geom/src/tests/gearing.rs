//! The gears that mesh with the spur gear (issue 123) are solids of the size they are given.

use super::*;
use crate::primitives::*;

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
