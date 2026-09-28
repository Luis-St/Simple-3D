//! Measuring to a world axis, where one is actually drawn.

use super::*;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_measure_click_catches_where_an_axis_crosses_a_body() {
    // Issue 78: where an axis leaves a body is measurable though the mesh has no corner there.
    let mut app = headless_app();
    let root = app.scene.root();
    let id = app.scene.add_primitive("box", root, 0).unwrap();
    // Off to one side, so only the crossing can answer, not the face centre.
    app.scene.get_mut(id).unwrap().position = Vec3::new(6.0, 0.0, 0.0);
    app.reevaluate_for_test();

    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let (_, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();
    let crossing = Vec3::new(0.0, 0.0, hi.z);
    let (screen, _) = view.project(crossing).unwrap();
    let point = app.measure_point_at(&view, screen).expect("a point under the cursor");
    assert_eq!(
        point.kind,
        Some(crate::snap::FeatureKind::AxisCrossing),
        "the Z axis leaving the top face was not catchable; caught {:?}",
        point.kind
    );
    assert!((point.at - crossing).length() < 1e-6);

    // With Z off there is nothing on screen to catch there.
    app.scene.settings.axes_visible[2] = false;
    let point = app.measure_point_at(&view, screen).unwrap();
    assert_ne!(
        point.kind,
        Some(crate::snap::FeatureKind::AxisCrossing),
        "an axis that is not shown was still snapped to"
    );
}

#[test]
pub(crate) fn a_measure_click_catches_a_world_axis_anywhere_along_it() {
    // Axes are caught anywhere along them, like edges.
    let mut app = headless_app();
    let root = app.scene.root();
    app.scene.add_primitive("box", root, 0).unwrap();
    app.reevaluate_for_test();
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);

    // Well clear of the box, out of any body feature's reach.
    let on_axis = Vec3::new(30.0, 0.0, 0.0);
    let (screen, _) = view.project(on_axis).unwrap();
    let point = app.measure_point_at(&view, screen).expect("a point under the cursor");
    assert_eq!(point.kind, Some(crate::snap::FeatureKind::Axis), "caught {:?}", point.kind);
    assert!((point.at - on_axis).length() < 0.2, "caught {:?}, not the point on the axis", point.at);
    // Exactly on the axis.
    assert!(point.at.y.abs() < 1e-6 && point.at.z.abs() < 1e-6, "{:?} is off the X axis", point.at);

    // Beside the line nothing catches, and the click falls to the ground.
    let beside = app.measure_point_at(&view, screen + egui::vec2(0.0, 40.0)).unwrap();
    assert_eq!(beside.kind, None, "the catch reached far past the axis");

    // A hidden axis is not catchable.
    app.scene.settings.axes_visible[0] = false;
    let hidden = app.measure_point_at(&view, screen).unwrap();
    assert_ne!(hidden.kind, Some(crate::snap::FeatureKind::Axis), "a hidden axis was still snapped to");
}

#[test]
pub(crate) fn an_axis_is_only_caught_where_it_is_actually_drawn() {
    // The catch stops at the surface, since the line is cut out of material and hidden behind bodies.
    let mut app = headless_app();
    let root = app.scene.root();
    let id = app.scene.add_primitive("box", root, 0).unwrap();
    app.reevaluate_for_test();
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let (lo, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();

    // Dead centre of a body the axes run through: no line there. Asked of the line pass directly,
    // since a nearby face centre would answer first.
    let middle = (lo + hi) * 0.5;
    let (screen, _) = view.project(middle).unwrap();
    assert!(
        app.nearest_line(&view, screen).is_none_or(|(_, kind, _, _)| kind != crate::snap::FeatureKind::Axis),
        "the catch followed an axis into the middle of a body, where no line is drawn"
    );
    let inside = app.measure_point_at(&view, screen).expect("a point under the cursor");
    assert!(
        !matches!(inside.kind, Some(crate::snap::FeatureKind::Axis | crate::snap::FeatureKind::AxisCrossing)),
        "an axis inside a body was caught: {inside:?}"
    );

    // The visible crossing is still caught.
    let near_top = Vec3::new(0.0, 0.0, hi.z);
    assert!(app.in_clear_view(&view, near_top), "the top crossing is covered in this view; pick another point");
    let (screen, _) = view.project(near_top).unwrap();
    let seen = app.measure_point_at(&view, screen).unwrap();
    assert!(seen.kind.is_some() && (seen.at - near_top).length() < 1e-6, "the visible crossing was lost: {seen:?}");

    // The underside crossing is hidden behind the body, as is the line arriving at it.
    let far = Vec3::new(0.0, 0.0, lo.z);
    assert!(!app.in_clear_view(&view, far), "the far crossing is not covered in this view; pick another point");
    let (screen, _) = view.project(far).unwrap();
    let hidden = app.measure_point_at(&view, screen).unwrap();
    assert!(
        !matches!(hidden.kind, Some(crate::snap::FeatureKind::Axis | crate::snap::FeatureKind::AxisCrossing)),
        "an axis behind the body was caught through it: {hidden:?}"
    );
}
