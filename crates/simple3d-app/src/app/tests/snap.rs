//! Snapping a move drag onto another body.

use super::*;
use crate::gizmo::Handle;
use simple3d_core::config::SnapMode;
use simple3d_core::keymap::{Chord, Command};
use simple3d_geom::Vec3;

#[test]
pub(crate) fn geometry_snapping_is_asked_for_by_the_mode_and_the_held_key() {
    // Issue 68's three modes, through the viewport's own call.
    let none = egui::Modifiers::NONE;
    let mut app = headless_app();
    app.settings.geometry_snap = SnapMode::Never;
    assert!(!app.geometry_snap_wanted(|_| true, none), "never should snap for no key");
    app.settings.geometry_snap = SnapMode::Always;
    assert!(app.geometry_snap_wanted(|_| false, none), "always should snap with no key held");

    app.settings.geometry_snap = SnapMode::WhileHeld;
    // The default hold is Ctrl alone (issue 77): the modifier state is the whole binding.
    assert_eq!(app.keymap.binding(Command::SnapToGeometry), Some(&Chord::modifiers(true, false, false)));
    assert!(!app.geometry_snap_wanted(|_| true, none), "held mode with no modifier down must not snap");
    assert!(app.geometry_snap_wanted(|_| false, egui::Modifiers::COMMAND), "Ctrl held must snap");
    assert!(
        !app.geometry_snap_wanted(|_| false, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT),
        "Ctrl+Shift is not the Ctrl binding"
    );

    // Rebound to a key, the key must be down and modifiers match exactly, so Ctrl+V (Paste) does not fire V.
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
    // Dragging the near box towards a corner of the far one lands exactly on it (issue 68).
    let (mut app, a, b) = two_boxes("snap-drag", Vec3::new(43.0, 0.0, 0.0));
    app.history.clear();
    app.reevaluate_for_test();

    // A real corner of B, from its evaluated world mesh.
    let (lo, hi) = app.evaluated.node_meshes[&b].bounds().unwrap();
    let corner = Vec3::new(lo.x, lo.y, hi.z);
    assert!(crate::snap::features_of(&app.evaluated.node_meshes[&b])
        .iter()
        .any(|f| f.kind == crate::snap::FeatureKind::Vertex && (f.point - corner).length() < 1e-6));

    app.settings.geometry_snap = SnapMode::Always;
    app.snap_requested = true;
    drag_gesture(&mut app, a, Handle::MoveAxis(0), corner, 3);

    // A corner of A caught B's corner, not A's origin, which left the boxes overlapping by half.
    app.reevaluate_for_test();
    let landed = crate::snap::features_of(&app.evaluated.node_meshes[&a])
        .iter()
        .any(|f| f.kind == crate::snap::FeatureKind::Vertex && (f.point - corner).length() < 1e-6);
    assert!(
        landed,
        "no corner of the dragged box met the target corner {corner:?}; it sits at {:?}",
        app.scene.node(a).position
    );
    // Off the 1 mm step, so only geometry could have produced it.
    assert!((app.scene.node(a).position.x - 23.0).abs() < 1e-6, "{:?}", app.scene.node(a).position);

    // An axis handle stays on its axis; the snap used to move Y and Z too.
    let after = app.scene.node(a).position;
    assert!(after.y.abs() < 1e-9 && after.z.abs() < 1e-9, "an X-axis drag moved in Y or Z: {after:?}");
}

#[test]
pub(crate) fn a_snapped_plane_drag_stays_in_its_plane() {
    // A plane handle moves in its two axes and leaves the third alone.
    let (mut app, a, b) = two_boxes("snap-plane", Vec3::new(43.0, 37.0, 25.0));
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

/// New meshes' snap targets are found off-thread and cached before the first snapping frame.
#[test]
pub(crate) fn an_evaluation_has_its_snap_targets_found_in_the_background() {
    let mut app = headless_app();
    let id = app.primary().unwrap();
    app.warm_snaps();
    let started = std::time::Instant::now();
    while app.snap_warming.is_some() && started.elapsed() < std::time::Duration::from_secs(10) {
        app.poll_snap_warming();
        std::thread::yield_now();
    }
    let mesh = app.evaluated.node_meshes.get(&id).unwrap().clone();
    let (_, _, mask) = app.snap_settings();
    let cached = app.snap_features.borrow();
    let (key, snaps) = cached.get(&id).expect("the body's targets were not found in the background");
    assert_eq!(*key, (std::sync::Arc::as_ptr(&mesh) as usize, mask));
    assert!(!snaps.features.is_empty());
    drop(cached);

    // With nothing new, nothing is started.
    app.warm_snaps();
    assert!(app.snap_warming.is_none());
}

#[test]
pub(crate) fn a_snap_marks_the_whole_face_two_bodies_meet_on() {
    // Issue 87: box A flush against B's -X face used to mark one corner; the four corners and edges
    // of that face are what meets.
    let (mut app, a, b) = two_boxes("snap-mark", Vec3::new(43.0, 0.0, 0.0));
    app.scene.get_mut(a).unwrap().position = Vec3::new(23.0, 0.0, 0.0);
    app.reevaluate_for_test();
    let moved: Vec<Vec3> = crate::snap::features_of(&app.evaluated.node_meshes[&a]).iter().map(|f| f.point).collect();
    let (lo, hi) = app.evaluated.node_meshes[&b].bounds().unwrap();
    let target = crate::snap::Feature::point(Vec3::new(lo.x, lo.y, hi.z), crate::snap::FeatureKind::Vertex);

    let mark = app.snap_mark(b, &target, &moved);
    assert_eq!(mark.corners.len(), 4, "the face met has four corners: {:?}", mark.corners);
    assert_eq!(mark.edges.len(), 4, "the face met has four edges: {:?}", mark.edges);
    let on_face = |p: Vec3| (p.x - lo.x).abs() < 1e-6;
    assert!(mark.corners.iter().all(|&p| on_face(p)), "a corner off the face met: {:?}", mark.corners);
    assert!(mark.edges.iter().all(|&(p, q)| on_face(p) && on_face(q)), "an edge off the face met: {:?}", mark.edges);

    // Apart, only the corner caught is marked.
    app.scene.get_mut(a).unwrap().position = Vec3::ZERO;
    app.reevaluate_for_test();
    let moved: Vec<Vec3> = crate::snap::features_of(&app.evaluated.node_meshes[&a]).iter().map(|f| f.point).collect();
    let mark = app.snap_mark(b, &target, &moved);
    assert_eq!((mark.corners, mark.edges.len()), (vec![target.point], 0));
}
