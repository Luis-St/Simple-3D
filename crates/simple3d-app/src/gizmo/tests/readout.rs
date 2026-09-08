//! What the manipulator reports while it is dragged.

use super::*;
use simple3d_core::unit::Unit;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn the_readout_is_in_the_display_unit() {
    let mut f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Move);
    let handle = Handle::MoveAxis(0);
    let from = f.view.project(gizmo.handle_point(handle, &f.view)).unwrap().0;
    let to = f.view.project(gizmo.handle_point(handle, &f.view) + Vec3::new(20.0, 0.0, 0.0)).unwrap().0;
    let mut drag = Drag::begin(&f.scene, &gizmo, f.node, handle, &f.view, from).unwrap();
    drag.update(&mut f.scene, &f.view, to, Mods::default(), 10.0, 15.0, Unit::Centimetre);
    assert_eq!(drag.readout, "X +2cm", "{}", drag.readout);
    drag.update(&mut f.scene, &f.view, to, Mods::default(), 10.0, 15.0, Unit::Millimetre);
    assert_eq!(drag.readout, "X +20mm");
}

#[test]
pub(crate) fn modifier_snapping_is_what_the_table_in_the_doc_comment_says() {
    assert_eq!(Mods::default().snap(23.0, 10.0), 20.0);
    assert_eq!(Mods { free: true, ..Default::default() }.snap(23.0, 10.0), 23.0);
    assert_eq!(Mods { coarse: true, ..Default::default() }.snap(63.0, 10.0), 100.0);
    // A zero increment cannot snap, and must not divide by zero.
    assert_eq!(Mods::default().snap(23.0, 0.0), 23.0);
}
