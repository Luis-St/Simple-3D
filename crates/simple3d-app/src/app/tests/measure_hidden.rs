//! A measure click never catches what the model hides.

use super::*;
use simple3d_core::config::DisplayMode;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_measure_click_never_catches_what_the_body_hides() {
    // The bug the screencast showed: aimed anywhere at the top face of a
    // box, the tool jumped to corners and edges on the far side of it. They
    // are nowhere near the pointer in the model, but a solid projects its own
    // back over its front, so their projections land in the middle of the
    // face being pointed at -- and nothing there is drawn, which is what made
    // the catch look random.
    let mut app = headless_app();
    let root = app.scene.root();
    for id in app.scene.descendants(root) {
        app.scene.remove(id);
    }
    let id = app.scene.add_primitive("box", root, 0).unwrap();
    // Looking well down on the box, so its underside projects across its top.
    app.scene.camera.pitch = 45.0;
    app.reevaluate_for_test();
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let (lo, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();

    // The far bottom corner is one of them, and it used to be what a pointer
    // three quarters of the way across the top face caught.
    let buried = Vec3::new(lo.x, hi.y, lo.z);
    assert!(!app.in_clear_view(&view, buried), "this corner is not hidden in this view; pick another");

    // Every point of the top face, and nothing caught anywhere on it is a
    // thing the frame does not show.
    for row in 1..8 {
        for column in 1..8 {
            let at =
                Vec3::new(lo.x + (hi.x - lo.x) * row as f64 / 8.0, lo.y + (hi.y - lo.y) * column as f64 / 8.0, hi.z);
            let (screen, _) = view.project(at).unwrap();
            let caught = app.measure_point_at(&view, screen).expect("the top face is under the cursor");
            assert!(
                (caught.at - buried).length() > 1e-6,
                "aiming at {at:?} on the top face caught the corner buried behind it"
            );
            assert!(app.in_clear_view(&view, caught.at), "aiming at {at:?} caught {caught:?}, which the body hides");
        }
    }
}

#[test]
pub(crate) fn wireframe_hides_nothing_and_so_withholds_nothing() {
    // The far side of a body is drawn in wireframe exactly like the near
    // side -- that is the point of the mode -- so a corner around the back is
    // a corner on screen there, and catching it is aiming at what is drawn
    // rather than at what is not.
    let mut app = headless_app();
    let root = app.scene.root();
    for id in app.scene.descendants(root) {
        app.scene.remove(id);
    }
    let id = app.scene.add_primitive("box", root, 0).unwrap();
    app.scene.camera.pitch = 45.0;
    app.reevaluate_for_test();
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let (lo, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();

    let buried = Vec3::new(lo.x, hi.y, lo.z);
    let (screen, _) = view.project(buried).unwrap();
    assert!(!app.in_clear_view(&view, buried));
    let shaded = app.measure_point_at(&view, screen).unwrap();
    assert!((shaded.at - buried).length() > 1e-6, "a shaded body handed out the corner behind it");

    app.settings.display_mode = DisplayMode::Wireframe;
    let wire = app.measure_point_at(&view, screen).expect("the corner is under the cursor");
    assert_eq!(wire.kind, Some(crate::snap::FeatureKind::Vertex), "caught {wire:?}");
    assert!((wire.at - buried).length() < 1e-6, "caught {:?}, not the corner {buried:?}", wire.at);
}

#[test]
pub(crate) fn a_measure_click_catches_the_mark_a_principal_plane_leaves_on_a_body() {
    // The line the renderer draws across the solid where a plane through the
    // origin cuts it is a line on screen like any other, and "how far along
    // this face is zero" is the measurement it exists to be read for. Until
    // now nothing was there to catch, and the pointer fell through it.
    let mut app = headless_app();
    let root = app.scene.root();
    for id in app.scene.descendants(root) {
        app.scene.remove(id);
    }
    let id = app.scene.add_primitive("box", root, 0).unwrap();
    // Off the origin on both of the ground's axes, so the mark on the top
    // face runs nowhere near that face's centre and only the mark can answer.
    app.scene.get_mut(id).unwrap().position = Vec3::new(5.0, 5.0, 0.0);
    app.reevaluate_for_test();
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let (_, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();

    // On the top face, on the line the x = 0 plane leaves across it, and well
    // clear of every corner, edge and centre of that face.
    let on_mark = Vec3::new(0.0, 10.0, hi.z);
    let (screen, _) = view.project(on_mark).unwrap();
    let caught = app.measure_point_at(&view, screen).expect("the top face is under the cursor");
    assert_eq!(caught.kind, Some(crate::snap::FeatureKind::PlaneMark), "caught {caught:?}");
    // Caught *on* the plane, not merely near it: that exactness is the whole
    // use of the mark.
    assert!(caught.at.x.abs() < 1e-6, "{:?} is off the x = 0 plane", caught.at);
    assert!((caught.at - on_mark).length() < 0.3, "caught {:?}, not the point pointed at", caught.at);

    // A little to the side of the mark there is no line to catch, and the
    // click lands on whatever is really there instead.
    let beside = app.measure_point_at(&view, screen + egui::vec2(30.0, 0.0)).unwrap();
    assert_ne!(beside.kind, Some(crate::snap::FeatureKind::PlaneMark), "the catch reached far past the mark");

    // The mark follows the switch of the axis it is *drawn as*, and the
    // switch for the marks themselves. Neither drawn nor caught.
    //
    // This one is left by the plane perpendicular to X, which is drawn in
    // Y's green and answers to the Y box: X's and Y's marks are exchanged on
    // purpose (`snap::MARK_AXIS`, issue 75).
    app.scene.settings.axes_visible[0] = false;
    let other = app.measure_point_at(&view, screen).unwrap();
    assert_eq!(other.kind, Some(crate::snap::FeatureKind::PlaneMark), "the wrong switch took this mark away");
    app.scene.settings.axes_visible[0] = true;
    app.scene.settings.axes_visible[1] = false;
    let hidden = app.measure_point_at(&view, screen).unwrap();
    assert_ne!(hidden.kind, Some(crate::snap::FeatureKind::PlaneMark), "a mark of a hidden plane was caught");
    app.scene.settings.axes_visible[1] = true;
    app.scene.settings.plane_marks = false;
    let off = app.measure_point_at(&view, screen).unwrap();
    assert_ne!(off.kind, Some(crate::snap::FeatureKind::PlaneMark), "a mark that is switched off was caught");

    // And there is no surface for one in wireframe, where none is drawn.
    app.scene.settings.plane_marks = true;
    app.settings.display_mode = DisplayMode::Wireframe;
    let wire = app.measure_point_at(&view, screen).unwrap();
    assert_ne!(wire.kind, Some(crate::snap::FeatureKind::PlaneMark), "a mark was caught where none is drawn");
}
