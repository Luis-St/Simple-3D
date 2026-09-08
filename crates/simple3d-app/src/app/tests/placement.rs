//! Where a new shape lands.

use super::*;
use simple3d_core::config::Placement;
use simple3d_core::keymap::Command;
use simple3d_core::scene::GroupOp;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_new_shape_lands_where_the_placement_says_and_the_hint_agrees() {
    let mut app = headless_app();
    assert_eq!(app.settings.placement, Placement::Origin);
    assert!(insertion_hint(&app).contains("origin"), "{}", insertion_hint(&app));
    app.add_node(Some("sphere"), GroupOp::Union);
    assert_eq!(app.scene.node(app.primary().unwrap()).position, Vec3::ZERO);

    app.settings.placement = Placement::Cursor;
    app.cursor = Some(Vec3::new(30.0, -10.0, 5.0));
    let hint = insertion_hint(&app);
    assert!(hint.contains("cursor") && hint.contains("30"), "{hint}");
    app.add_node(Some("sphere"), GroupOp::Union);
    assert_eq!(app.scene.node(app.primary().unwrap()).position, Vec3::new(30.0, -10.0, 5.0));

    // What the camera is looking at, rounded to the step.
    app.settings.placement = Placement::ViewCentre;
    app.scene.settings.snap_step = 5.0;
    app.scene.camera.target = Vec3::new(12.0, -3.0, 0.0);
    assert_eq!(app.insertion_point_world(0.0), Vec3::new(10.0, -5.0, 0.0));
    assert!(insertion_hint(&app).contains("camera"), "{}", insertion_hint(&app));

    // Beside the selection: clear of it, not inside it.
    app.clear_selection();
    let plate = app.scene.depth_first().into_iter().find(|&id| id != app.scene.root()).unwrap();
    app.select_only(plate);
    app.reevaluate_for_test();
    app.settings.placement = Placement::BesideSelection;
    let (_, hi) = app.selection_bounds().expect("the plate has bounds");
    assert_eq!(app.insertion_point_world(0.0).x, hi.x + app.move_snap());
}

/// The point of "beside the selection" is that the two do not overlap. An
/// assertion on the formula cannot see that -- it restates it -- so this
/// adds the shape and measures where it actually ended up.
#[test]
pub(crate) fn a_shape_added_beside_the_selection_does_not_overlap_it() {
    for type_id in ["box", "sphere", "tetrahedron", "cylinder"] {
        let mut app = headless_app();
        app.settings.placement = Placement::Origin;
        app.add_node(Some(type_id), GroupOp::Union);
        let first = app.primary().unwrap();
        app.reevaluate_for_test();
        let (_, hi) = app.selection_bounds().expect("the first shape has bounds");

        app.settings.placement = Placement::BesideSelection;
        app.add_node(Some(type_id), GroupOp::Union);
        let second = app.primary().unwrap();
        assert_ne!(first, second);
        app.reevaluate_for_test();
        let (lo2, _) = app.selection_bounds().expect("the second shape has bounds");

        assert!(
            lo2.x >= hi.x,
            "a {type_id} added beside the selection starts at {} but the selection reaches {}",
            lo2.x,
            hi.x
        );
        // One step of air between them, no more and no less.
        assert!(
            (lo2.x - hi.x - app.move_snap()).abs() < 1e-6,
            "a {type_id} left {} of air, not one step of {}",
            lo2.x - hi.x,
            app.move_snap()
        );
    }
}

#[test]
pub(crate) fn a_shape_added_into_a_rotated_group_still_lands_where_the_placement_says() {
    // `Node::position` is in the parent's coordinates. Writing a world point
    // into it unchanged would put the shape somewhere else entirely as soon
    // as the group it goes into is turned or moved.
    let mut app = headless_app();
    let plate = app.primary().unwrap();
    app.select_only(plate);
    app.run(Command::Group);
    let group = app.primary().unwrap();
    if let Some(node) = app.scene.get_mut(group) {
        node.position = Vec3::new(50.0, 0.0, 0.0);
        node.rotation = Vec3::new(0.0, 0.0, 90.0);
    }
    app.reevaluate_for_test();

    app.settings.placement = Placement::Cursor;
    app.cursor = Some(Vec3::new(10.0, 0.0, 0.0));
    app.select_only(group);
    app.add_node(Some("sphere"), GroupOp::Union);
    let added = app.primary().unwrap();
    assert_eq!(app.scene.node(added).parent, Some(group), "the sphere did not go into the group");

    app.reevaluate_for_test();
    let world = app.evaluated.node_meshes[&added].bounds().expect("the sphere has bounds");
    let centre = (world.0 + world.1) * 0.5;
    assert!((centre - Vec3::new(10.0, 0.0, 0.0)).length() < 1e-6, "the sphere landed at {centre:?}");
}
