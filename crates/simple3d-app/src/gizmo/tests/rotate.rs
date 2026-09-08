//! Rotate drags and the angles they land on.

use super::*;
use simple3d_core::unit::Unit;

#[test]
pub(crate) fn a_rotate_drag_snaps_to_fifteen_degrees_by_default() {
    let mut f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Rotate);
    let ring = gizmo.ring_points(2, &f.view, 72);
    let from = f.view.project(ring[0]).unwrap().0;
    // A little over 30 degrees around the ring.
    let to = f.view.project(ring[7]).unwrap().0;
    let mut drag = Drag::begin(&f.scene, &gizmo, f.node, Handle::RotateRing(2), &f.view, from).unwrap();
    drag.update(&mut f.scene, &f.view, to, Mods::default(), 10.0, 15.0, Unit::Millimetre);
    let z = f.scene.node(f.node).rotation.z;
    assert!((z % 15.0).abs() < 1e-9, "not snapped to 15 degrees: {z}");
    assert!(z > 0.0, "rotated the wrong way: {z}");
    assert!(drag.readout.contains("deg"), "{}", drag.readout);
}

/// Issue 84: the ring counts turns without end -- that is what makes a drag
/// across the seam keep going the way it was going -- but neither the model
/// nor the readout beside the pointer is a count of turns. Dragged twice
/// round, the rotation used to read 725 in the property panel and "Z 725deg"
/// at the cursor.
#[test]
pub(crate) fn a_rotate_drag_of_more_than_a_turn_leaves_a_rotation_inside_one() {
    let mut f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Rotate);
    let ring = gizmo.ring_points(2, &f.view, 72);
    let from = f.view.project(ring[0]).expect("the ring is off screen").0;
    let mut drag = Drag::begin(&f.scene, &gizmo, f.node, Handle::RotateRing(2), &f.view, from).unwrap();
    // Twice round the ring, and fifteen degrees more: 735 degrees of turn.
    for step in 1..=(72 * 2 + 3) {
        let to = f.view.project(ring[step % 72]).expect("the ring is off screen").0;
        drag.update(&mut f.scene, &f.view, to, Mods::default(), 10.0, 15.0, Unit::Millimetre);
    }

    let z = f.scene.node(f.node).rotation.z;
    assert!((0.0..360.0).contains(&z), "a drag of more than a turn left the rotation at {z}");
    assert!((z - 15.0).abs() < 1e-6, "735 degrees of turn is 15 degrees of rotation, not {z}");

    // And the readout says how far the ring went, through the same wrap.
    let turned: f64 = drag
        .readout
        .trim_start_matches("Z ")
        .trim_end_matches("deg")
        .parse()
        .unwrap_or_else(|_| panic!("the readout is not a number: {}", drag.readout));
    assert!((0.0..360.0).contains(&turned), "the readout is outside a single turn: {}", drag.readout);
    assert!((turned - 15.0).abs() < 1e-6, "{}", drag.readout);
}

/// The readout and the rotation field have to say the same kind of number.
/// The field cannot show a negative -- a rotation is a direction, and a
/// direction is written as one turn from zero -- so neither does this.
#[test]
pub(crate) fn a_rotate_drag_backwards_reads_the_way_the_rotation_field_does() {
    let mut f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Rotate);
    let ring = gizmo.ring_points(2, &f.view, 72);
    let from = f.view.project(ring[0]).expect("the ring is off screen").0;
    let mut drag = Drag::begin(&f.scene, &gizmo, f.node, Handle::RotateRing(2), &f.view, from).unwrap();
    // Three points the other way round the ring: fifteen degrees back.
    for step in [71, 70, 69] {
        let to = f.view.project(ring[step]).expect("the ring is off screen").0;
        drag.update(&mut f.scene, &f.view, to, Mods::default(), 10.0, 15.0, Unit::Millimetre);
    }

    let z = f.scene.node(f.node).rotation.z;
    assert!((z - 345.0).abs() < 1e-6, "a turn backwards from zero is 345 degrees, not {z}");
    assert_eq!(drag.readout, "Z 345deg", "the readout went negative where the field cannot");
}

#[test]
pub(crate) fn a_free_rotate_drag_is_not_snapped() {
    let mut f = Fixture::new("box");
    let gizmo = f.gizmo(Mode::Rotate);
    let ring = gizmo.ring_points(2, &f.view, 72);
    let from = f.view.project(ring[0]).unwrap().0;
    let to = f.view.project(ring[7]).unwrap().0;
    let mut drag = Drag::begin(&f.scene, &gizmo, f.node, Handle::RotateRing(2), &f.view, from).unwrap();
    drag.update(&mut f.scene, &f.view, to, Mods { free: true, ..Default::default() }, 10.0, 15.0, Unit::Millimetre);
    let z = f.scene.node(f.node).rotation.z;
    assert!(z > 20.0 && z < 45.0, "{z}");
    assert!((z % 15.0).abs() > 1e-6, "a free drag snapped anyway: {z}");
}
