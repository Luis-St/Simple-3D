//! Snapping a move drag onto another body.

use super::*;
use crate::gizmo::Handle;
use simple3d_core::config::SnapMode;
use simple3d_core::keymap::{Chord, Command};
use simple3d_geom::Vec3;

#[test]
pub(crate) fn geometry_snapping_is_asked_for_by_the_mode_and_the_held_key() {
    // Issue 68's three modes, resolved through the same call the viewport
    // makes -- the key-down closure standing in for the live keyboard.
    let none = egui::Modifiers::NONE;
    let mut app = headless_app();
    app.settings.geometry_snap = SnapMode::Never;
    assert!(!app.geometry_snap_wanted(|_| true, none), "never should snap for no key");
    app.settings.geometry_snap = SnapMode::Always;
    assert!(app.geometry_snap_wanted(|_| false, none), "always should snap with no key held");

    app.settings.geometry_snap = SnapMode::WhileHeld;
    // The default hold is Ctrl on its own (issue 77): no key to hold, the
    // modifier state is the whole binding.
    assert_eq!(app.keymap.binding(Command::SnapToGeometry), Some(&Chord::modifiers(true, false, false)));
    assert!(!app.geometry_snap_wanted(|_| true, none), "held mode with no modifier down must not snap");
    assert!(app.geometry_snap_wanted(|_| false, egui::Modifiers::COMMAND), "Ctrl held must snap");
    assert!(
        !app.geometry_snap_wanted(|_| false, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT),
        "Ctrl+Shift is not the Ctrl binding"
    );

    // Rebound to a key, the key has to be down and the modifiers have to
    // match exactly. A hold on Ctrl+V must not fire on a bare V -- and the
    // unmodified binding must not fire on Ctrl+V, which is Paste.
    app.keymap.set(Command::SnapToGeometry, Chord::key("V"), true).unwrap();
    let key = crate::ui::key_from_name("V").unwrap();
    assert!(!app.geometry_snap_wanted(|_| false, none), "held mode with nothing down must not snap");
    assert!(app.geometry_snap_wanted(|k| k == key, none), "held mode with the snap key down must snap");
    assert!(!app.geometry_snap_wanted(|k| k == key, egui::Modifiers::COMMAND), "the bare binding fired with Ctrl down");
    app.keymap.set(Command::SnapToGeometry, Chord::ctrl("V"), true).unwrap();
    assert!(!app.geometry_snap_wanted(|k| k == key, none), "a Ctrl+V hold fired on a bare V");
    assert!(app.geometry_snap_wanted(|k| k == key, egui::Modifiers::COMMAND));
}

#[test]
pub(crate) fn a_move_drag_snaps_a_body_onto_another_bodys_vertex() {
    // Two boxes; drag the near one toward a corner of the far one with
    // geometry snapping asked for, and it lands exactly on that corner rather
    // than on the grid (issue 68).
    let mut app = app_in(temp_config_dir("snap-drag"));
    let root = app.scene.root();
    let a = app.scene.add_primitive("box", root, 0).unwrap();
    let b = app.scene.add_primitive("box", root, 1).unwrap();
    app.scene.get_mut(b).unwrap().position = Vec3::new(43.0, 0.0, 0.0);
    app.select_only(a);
    app.history.clear();
    app.reevaluate_for_test();

    // A real corner of B, taken from its evaluated world mesh.
    let (lo, hi) = app.evaluated.node_meshes[&b].bounds().unwrap();
    let corner = Vec3::new(lo.x, lo.y, hi.z);
    assert!(crate::snap::features_of(&app.evaluated.node_meshes[&b])
        .iter()
        .any(|f| f.kind == crate::snap::FeatureKind::Vertex && (f.point - corner).length() < 1e-6));

    app.settings.geometry_snap = SnapMode::Always;
    app.snap_requested = true;
    drag_gesture(&mut app, a, Handle::MoveAxis(0), corner, 3);

    // A *corner of A* caught the corner of B -- not A's origin. Snapping used
    // to move the origin onto the target, which left two boxes overlapping by
    // half rather than meeting at a corner.
    app.reevaluate_for_test();
    let landed = crate::snap::features_of(&app.evaluated.node_meshes[&a])
        .iter()
        .any(|f| f.kind == crate::snap::FeatureKind::Vertex && (f.point - corner).length() < 1e-6);
    assert!(
        landed,
        "no corner of the dragged box met the target corner {corner:?}; it sits at {:?}",
        app.scene.node(a).position
    );
    // 43 is not a multiple of the 1mm step, so a grid-only drag could not
    // have produced this.
    assert!((app.scene.node(a).position.x - 23.0).abs() < 1e-6, "{:?}", app.scene.node(a).position);

    // And an axis handle stayed on its axis. The whole point of grabbing the
    // X arrow is that Y and Z do not move; the snap used to overwrite all
    // three and slide the body off to (53, -10, 10).
    let after = app.scene.node(a).position;
    assert!(after.y.abs() < 1e-9 && after.z.abs() < 1e-9, "an X-axis drag moved in Y or Z: {after:?}");
}

#[test]
pub(crate) fn a_snapped_plane_drag_stays_in_its_plane() {
    // The same constraint for the other move handle: a plane handle may move
    // in its two axes and must leave the third alone.
    let mut app = app_in(temp_config_dir("snap-plane"));
    let root = app.scene.root();
    let a = app.scene.add_primitive("box", root, 0).unwrap();
    let b = app.scene.add_primitive("box", root, 1).unwrap();
    app.scene.get_mut(b).unwrap().position = Vec3::new(43.0, 37.0, 25.0);
    app.select_only(a);
    app.history.clear();
    app.reevaluate_for_test();
    let (lo, hi) = app.evaluated.node_meshes[&b].bounds().unwrap();
    let corner = Vec3::new(lo.x, lo.y, hi.z);

    app.settings.geometry_snap = SnapMode::Always;
    app.snap_requested = true;
    // MovePlane(2): free in X and Y, pinned in Z.
    drag_gesture(&mut app, a, Handle::MovePlane(2), corner, 3);
    let after = app.scene.node(a).position;
    assert!(after.z.abs() < 1e-9, "a Z-plane drag moved in Z: {after:?}");
    assert!(after.x.abs() > 1e-6 && after.y.abs() > 1e-6, "the drag did not snap at all: {after:?}");
}
