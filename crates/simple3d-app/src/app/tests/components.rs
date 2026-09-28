//! Components: several node trees in one project, each in its own tab (issue 113).

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::scene::{GroupOp, NodeId, ROOT_COMPONENT};
use simple3d_geom::Vec3;

/// A root component with one selected union group of two boxes.
pub(super) fn app_with_group(name: &str) -> (App, NodeId) {
    let mut app = app_in(temp_config_dir(name));
    let root = app.scene.root();
    let group = app.scene.add_group(GroupOp::Union, root, 0);
    app.scene.get_mut(group).unwrap().name = "Wheel".into();
    app.scene.add_primitive("box", group, 0).unwrap();
    let second = app.scene.add_primitive("box", group, 1).unwrap();
    app.scene.get_mut(second).unwrap().position = Vec3::new(15.0, 0.0, 0.0);
    app.scene.get_mut(group).unwrap().position = Vec3::new(0.0, 40.0, 0.0);
    app.select_only(group);
    app.history.clear();
    app.saved_revision = app.history.revision();
    app.reevaluate_for_test();
    (app, group)
}

pub(super) fn bounds(app: &App) -> (Vec3, Vec3) {
    app.evaluated.bounds.expect("the model evaluated to nothing")
}

pub(super) fn close(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    (a.0 - b.0).length() < 1e-6 && (a.1 - b.1).length() < 1e-6
}

/// Making a component of a group leaves one childless node and an unchanged model.
#[test]
pub(crate) fn making_a_component_leaves_one_node_and_the_same_model() {
    let (mut app, group) = app_with_group("components-make");
    let before = bounds(&app);
    assert!(!app.project.uses_components());

    app.run(Command::MakeComponent);
    app.reevaluate_for_test();

    let node = app.scene.node(group);
    assert!(node.is_component(), "the group did not become an integration");
    assert!(node.children.is_empty(), "an integration shows what is inside it in the tree");
    assert_eq!(node.position, Vec3::new(0.0, 40.0, 0.0), "the integration lost the group's place");
    assert!(app.project.uses_components());
    let component = app.scene.component_of(group).unwrap();
    assert_eq!(app.component_label(component).as_deref(), Some("Wheel"));
    assert!(close(before, bounds(&app)), "the model moved: {before:?} against {:?}", bounds(&app));
    assert!(app.evaluated.errors.is_empty(), "{:?}", app.evaluated.errors);
}

/// An edit in a component's tab shows in every integration, and the tab shows the component alone.
#[test]
pub(crate) fn an_edit_in_the_component_shows_up_everywhere_it_is_placed() {
    let (mut app, group) = app_with_group("components-edit");
    app.run(Command::MakeComponent);
    app.run(Command::Duplicate);
    let copy = app.primary().unwrap();
    assert_ne!(copy, group);
    assert_eq!(app.scene.component_of(copy), app.scene.component_of(group), "a duplicate is a second placement");
    app.scene.get_mut(copy).unwrap().position = Vec3::new(0.0, -40.0, 0.0);
    app.reevaluate_for_test();
    let triangles = app.evaluated.mesh.triangle_count();

    let component = app.scene.component_of(group).unwrap();
    app.activate_component(component);
    assert_eq!(app.project.active, component);
    assert_eq!(app.project.open, vec![ROOT_COMPONENT, component], "the component did not get a tab");
    app.reevaluate_for_test();
    assert_eq!(app.evaluated.mesh.triangle_count() * 2, triangles, "the tab does not show the component alone");
    let root = app.scene.root();
    app.edit("Add", None);
    let sphere = app.scene.add_primitive("sphere", root, 2).unwrap();
    app.scene.get_mut(sphere).unwrap().position = Vec3::new(0.0, 0.0, 60.0);

    app.activate_component(ROOT_COMPONENT);
    app.reevaluate_for_test();
    assert!(
        app.evaluated.mesh.triangle_count() > triangles,
        "the sphere added to the component is not in either placement of it"
    );
    assert!(app.evaluated.errors.is_empty(), "{:?}", app.evaluated.errors);
    // Each tab has its own history.
    assert_eq!(app.history.undo_label(), Some("Duplicate"));
}

/// A component can never end up inside itself, directly or through another.
#[test]
pub(crate) fn a_component_cannot_be_placed_inside_itself() {
    let (mut app, group) = app_with_group("components-cycle");
    app.run(Command::MakeComponent);
    let wheel = app.scene.component_of(group).unwrap();
    app.activate_component(wheel);
    assert!(app.can_integrate(wheel).is_err(), "a component was offered to itself");
    assert!(app.can_integrate(ROOT_COMPONENT).is_err(), "the root was offered as a component");

    app.new_component();
    let axle = app.project.active;
    {
        let root = app.scene.root();
        app.integrate_component_at(root, wheel);
    }
    assert_eq!(app.scene.integrations_of(wheel).len(), 1);
    app.activate_component(wheel);
    assert!(app.can_integrate(axle).is_err(), "a component holding this one was offered to it");
    let before = app.scene.len();
    {
        let root = app.scene.root();
        app.integrate_component_at(root, axle);
    }
    assert_eq!(app.scene.len(), before, "the refused placement was made anyway");
}

/// Deleting a component removes all its integrations everywhere, after asking.
#[test]
pub(crate) fn deleting_a_component_removes_every_integration_of_it() {
    let (mut app, group) = app_with_group("components-delete");
    app.run(Command::MakeComponent);
    let wheel = app.scene.component_of(group).unwrap();
    app.new_component();
    let axle = app.project.active;
    {
        let root = app.scene.root();
        app.integrate_component_at(root, wheel);
    }
    app.activate_component(ROOT_COMPONENT);

    app.ask_delete_component(wheel);
    assert_eq!(app.modal, Modal::ConfirmComponent);
    draw_one_frame(&mut app);
    app.confirm_component_ask();
    assert!(app.project.get(wheel).is_none());
    assert!(!app.scene.contains(group), "the integration in the root outlived its component");
    app.activate_component(axle);
    assert!(app.scene.integrations_of(wheel).is_empty(), "the integration in another component outlived it");
}

/// The window draws with the component tab row, an integration selected, and on a component tab.
#[test]
pub(crate) fn the_window_draws_with_components() {
    let (mut app, group) = app_with_group("components-draw");
    draw_one_frame(&mut app);
    app.run(Command::MakeComponent);
    app.select_only(group);
    draw_one_frame(&mut app);
    let component = app.scene.component_of(group).unwrap();
    app.activate_component(component);
    draw_one_frame(&mut app);
    app.close_component(component);
    assert_eq!(app.project.active, ROOT_COMPONENT);
    assert_eq!(app.project.open, vec![ROOT_COMPONENT]);
    assert!(app.project.get(component).is_some(), "closing the tab deleted the component");
}

/// A new component with a group selected takes that group, leaves an integration in its place,
/// and opens its tab while the view stays on the integration.
#[test]
pub(crate) fn a_new_component_with_a_group_selected_takes_the_group() {
    let (mut app, group) = app_with_group("components-new-from-group");
    app.run(Command::NewComponent);

    assert_eq!(app.project.active, ROOT_COMPONENT, "the view left the integration");
    let node = app.scene.node(group);
    assert!(node.is_component(), "the group was not taken into the component");
    assert!(node.children.is_empty());
    let component = app.scene.component_of(group).unwrap();
    assert_eq!(app.project.open, vec![ROOT_COMPONENT, component], "the component's tab did not open");
}

/// A component placed from a group row's Add menu goes inside that group.
#[test]
pub(crate) fn a_component_placed_on_a_group_row_goes_inside_it() {
    let (mut app, group) = app_with_group("components-place-on-row");
    app.run(Command::MakeComponent);
    let component = app.scene.component_of(group).unwrap();
    let root = app.scene.root();
    let holder = app.scene.add_group(GroupOp::Union, root, 0);

    app.integrate_component_at(holder, component);

    let placed = app.primary().unwrap();
    assert_eq!(app.scene.component_of(placed), Some(component));
    assert_eq!(app.scene.node(placed).parent, Some(holder), "the component was not placed inside the row's group");
}

/// A group with its default name becomes a numbered component, renaming the integration too;
/// a typed name is kept.
#[test]
pub(crate) fn a_group_with_its_given_name_becomes_a_numbered_component() {
    // An empty component first, so the numbering has one to count past.
    let (mut app, group) = app_with_group("components-default-name");
    app.clear_selection();
    app.new_component();
    app.activate_component(ROOT_COMPONENT);
    let root = app.scene.root();
    let mut made = Vec::new();
    for given in ["Group", "Group 3", "Wheel"] {
        let group = app.scene.add_group(GroupOp::Union, root, 0);
        app.scene.get_mut(group).unwrap().name = given.into();
        app.scene.add_primitive("box", group, 0).unwrap();
        app.select_only(group);
        app.run(Command::MakeComponent);
        made.push(group);
    }

    let names: Vec<String> = made.iter().map(|&node| app.scene.node(node).name.clone()).collect();
    assert_eq!(names, ["Component 2", "Component 3", "Wheel"], "the node left in place was named wrong");
    for &node in &made {
        let component = app.scene.component_of(node).unwrap();
        assert_eq!(app.component_label(component), Some(app.scene.node(node).name.clone()));
    }
    assert_eq!(app.scene.node(group).name, "Wheel");
    assert!(!app.scene.node(group).is_component());
}
