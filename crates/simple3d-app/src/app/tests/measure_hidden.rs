//! A measure click never catches what the model hides.

use super::*;
use simple3d_core::config::DisplayMode;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_measure_click_never_catches_what_the_body_hides() {
    // Regression: aiming at a box's top face caught far-side corners and edges whose projections land
    // on it.
    let mut app = headless_app();
    let root = app.scene.root();
    for id in app.scene.descendants(root) {
        app.scene.remove(id);
    }
    let id = app.scene.add_primitive("box", root, 0).unwrap();
    // Looking down, so the underside projects across the top.
    app.scene.camera.pitch = 45.0;
    app.reevaluate_for_test();
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let (lo, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();

    // The far bottom corner, which a pointer on the top face used to catch.
    let buried = Vec3::new(lo.x, hi.y, lo.z);
    assert!(!app.in_clear_view(&view, buried), "this corner is not hidden in this view; pick another");

    // Across the top face, nothing caught may be hidden.
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
    // Wireframe draws the far side, so catching it there is correct.
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
    // The principal plane mark is a catchable line like any other.
    let mut app = headless_app();
    let root = app.scene.root();
    for id in app.scene.descendants(root) {
        app.scene.remove(id);
    }
    let id = app.scene.add_primitive("box", root, 0).unwrap();
    // Off the origin on both ground axes, so only the mark can answer.
    app.scene.get_mut(id).unwrap().position = Vec3::new(5.0, 5.0, 0.0);
    app.reevaluate_for_test();
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let (_, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();

    // On the x = 0 mark across the top face, clear of every corner, edge and centre.
    let on_mark = Vec3::new(0.0, 10.0, hi.z);
    let (screen, _) = view.project(on_mark).unwrap();
    let caught = app.measure_point_at(&view, screen).expect("the top face is under the cursor");
    assert_eq!(caught.kind, Some(crate::snap::FeatureKind::PlaneMark), "caught {caught:?}");
    // Caught exactly on the plane.
    assert!(caught.at.x.abs() < 1e-6, "{:?} is off the x = 0 plane", caught.at);
    assert!((caught.at - on_mark).length() < 0.3, "caught {:?}, not the point pointed at", caught.at);

    // Beside the mark, the click lands on what is really there.
    let beside = app.measure_point_at(&view, screen + egui::vec2(30.0, 0.0)).unwrap();
    assert_ne!(beside.kind, Some(crate::snap::FeatureKind::PlaneMark), "the catch reached far past the mark");

    // The mark follows the switch of the axis it is drawn as (X and Y swapped, `snap::MARK_AXIS`,
    // issue 75) and the marks switch; off, it is neither drawn nor caught.
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

    // None in wireframe, where no surface is drawn.
    app.scene.settings.plane_marks = true;
    app.settings.display_mode = DisplayMode::Wireframe;
    let wire = app.measure_point_at(&view, screen).unwrap();
    assert_ne!(wire.kind, Some(crate::snap::FeatureKind::PlaneMark), "a mark was caught where none is drawn");
}
