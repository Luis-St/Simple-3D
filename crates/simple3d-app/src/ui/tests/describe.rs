//! The wording the interface asks for.

use super::*;
use simple3d_core::unit::Unit;

#[test]
pub(crate) fn a_bounding_box_reads_as_three_dimensions_in_the_display_unit() {
    let size = simple3d_geom::Vec3::new(40.0, 20.0, 4.0);
    assert_eq!(describe_size(size, Unit::Millimetre), "40 x 20 x 4 mm");
    assert_eq!(describe_size(size, Unit::Metre), "0.04 x 0.02 x 0.004 m");
}

#[test]
pub(crate) fn counts_and_durations_read_naturally() {
    assert_eq!(describe_counts(1, 1), "1 node  1 triangle");
    assert_eq!(describe_counts(3, 240), "3 nodes  240 triangles");
    assert_eq!(describe_elapsed(std::time::Duration::from_millis(12)), "12 ms");
    // Issue 38: the readout used to say "0 ms" for every evaluation that
    // took less than half a millisecond, which is most of them.
    assert_eq!(describe_elapsed(std::time::Duration::from_micros(240)), "0.24 ms");
    assert_eq!(describe_elapsed(std::time::Duration::from_millis(2500)), "2.5 s");
}
