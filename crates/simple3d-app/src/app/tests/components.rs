//! Components: a project of several node trees, each in a tab of its own
//! (issue 113).

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::project;
use simple3d_core::scene::{GroupOp, NodeId, ROOT_COMPONENT};
use simple3d_geom::Vec3;

/// A root component holding one union group of two boxes, the group selected.
fn app_with_group(name: &str) -> (App, NodeId) {
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

fn bounds(app: &App) -> (Vec3, Vec3) {
    app.evaluated.bounds.expect("the model evaluated to nothing")
}

fn close(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    (a.0 - b.0).length() < 1e-6 && (a.1 - b.1).length() < 1e-6
}

/// Making a component of a group leaves one node where the group was -- no
/// children in the tree -- and a model that looks exactly as it did.
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

/// An edit made in a component's own tab shows up in every integration of it,
/// and the tab shows the component on its own.
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
    // On its own: the two boxes, not the root's two placements of them.
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
    // Each tab has a history of its own: the root's last step is still the
    // duplicate, not the sphere.
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

    // A second component holding the wheel cannot then go into the wheel.
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

/// Undoing the making of a component takes the component away; redoing it
/// brings it back. Nothing is asked while there is nothing to lose.
#[test]
pub(crate) fn undoing_the_making_takes_the_component_away_and_redo_brings_it_back() {
    let (mut app, group) = app_with_group("components-undo");
    app.run(Command::MakeComponent);
    let component = app.scene.component_of(group).unwrap();

    app.run(Command::Undo);
    assert_eq!(app.modal, Modal::None, "an untouched component asked before it was taken away");
    assert!(app.scene.node(group).is_group(), "the undo did not give the group back");
    assert_eq!(app.scene.node(group).children.len(), 2);
    assert!(app.project.get(component).is_none(), "the component outlived the undo that took it away");
    assert!(!app.project.uses_components());

    app.run(Command::Redo);
    assert!(app.scene.node(group).is_component());
    assert!(app.project.get(component).is_some(), "the redo did not bring the component back");
    app.reevaluate_for_test();
    assert!(app.evaluated.errors.is_empty(), "{:?}", app.evaluated.errors);
}

/// Undoing the making of a component that has been worked on asks first, and
/// throws the work away only when told to.
#[test]
pub(crate) fn undoing_the_making_of_an_edited_component_asks_first() {
    let (mut app, group) = app_with_group("components-undo-edited");
    app.run(Command::MakeComponent);
    let component = app.scene.component_of(group).unwrap();
    app.activate_component(component);
    let root = app.scene.root();
    app.edit("Add", None);
    app.scene.add_primitive("sphere", root, 0).unwrap();
    app.activate_component(ROOT_COMPONENT);

    app.run(Command::Undo);
    assert_eq!(app.modal, Modal::ConfirmComponent, "the edited component was thrown away without asking");
    assert!(app.scene.node(group).is_component(), "the undo happened before it was answered");
    draw_one_frame(&mut app);
    app.cancel_component_ask();
    assert!(app.project.get(component).is_some());

    app.run(Command::Undo);
    app.confirm_component_ask();
    assert!(app.scene.node(group).is_group());
    assert!(app.project.get(component).is_none());
}

/// Deleting a component takes every integration of it with it, in every
/// component, after asking.
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

/// A project with components is written whole and read back whole; one
/// without is still the file it always was.
#[test]
pub(crate) fn a_project_with_components_saves_and_opens_whole() {
    let dir = temp_config_dir("components-file");
    let (mut app, group) = app_with_group("components-file-app");
    let plain = dir.join("plain.simple3d");
    app.save_to(&plain);
    assert!(std::fs::read_to_string(&plain).unwrap().contains(&format!("\"format\": {}", project::PLAIN_FORMAT)));

    app.run(Command::MakeComponent);
    let component = app.scene.component_of(group).unwrap();
    // Saved from the component's own tab, which must not matter.
    app.activate_component(component);
    assert!(app.unsaved());
    let path = dir.join("parts.simple3d");
    app.save_to(&path);
    assert!(!app.unsaved(), "saving left the project marked as changed");
    app.activate_component(ROOT_COMPONENT);
    assert!(!app.unsaved(), "the root component was not saved with the rest");
    app.reevaluate_for_test();
    let before = bounds(&app);

    let mut fresh = app_in(dir);
    fresh.open_path(&path);
    assert_eq!(fresh.project.components.len(), 2, "the component did not come back");
    assert!(fresh.scene.node(fresh.scene.root()).children.iter().any(|&id| fresh.scene.node(id).is_component()));
    fresh.reevaluate_for_test();
    assert!(close(before, bounds(&fresh)), "the reopened project evaluates differently");
    assert!(fresh.evaluated.errors.is_empty(), "{:?}", fresh.evaluated.errors);
}

/// A saved primitive placed twice is two components: taking something off the
/// shelf twice does not link the two (issue 113).
#[test]
pub(crate) fn every_placed_primitive_is_a_component_of_its_own() {
    let (mut app, group) = app_with_group("components-primitive");
    app.select_only(group);
    app.save_selection_as_primitive();
    app.confirm_save_primitive();
    let entry = app.library[0].clone();

    app.add_library_entry(&entry);
    let first = app.scene.component_of(app.primary().unwrap()).expect("the primitive is not a component");
    app.add_library_entry(&entry);
    let second = app.scene.component_of(app.primary().unwrap()).expect("the primitive is not a component");
    assert_ne!(first, second, "two placements of a primitive share one component");
    app.reevaluate_for_test();
    assert!(app.evaluated.errors.is_empty(), "{:?}", app.evaluated.errors);

    // And undoing a placement takes its component with it.
    app.run(Command::Undo);
    assert!(app.project.get(second).is_none());
}

/// Copy and paste inside a project places the same component again; into
/// another project it brings a copy of it along.
#[test]
pub(crate) fn a_pasted_integration_links_here_and_copies_elsewhere() {
    let (mut app, group) = app_with_group("components-paste");
    app.run(Command::MakeComponent);
    let component = app.scene.component_of(group).unwrap();
    app.select_only(group);
    app.run(Command::Copy);
    app.run(Command::Paste);
    let pasted = app.primary().unwrap();
    assert_eq!(app.scene.component_of(pasted), Some(component), "a paste in the same project made a copy");

    app.run(Command::New);
    app.run(Command::Paste);
    let there = app.primary().expect("nothing was pasted into the other project");
    let copied = app.scene.component_of(there).unwrap();
    assert!(app.project.get(copied).is_some(), "the pasted integration arrived without its component");
    app.reevaluate_for_test();
    assert!(app.evaluated.errors.is_empty(), "{:?}", app.evaluated.errors);
    assert!(app.evaluated.bounds.is_some(), "the pasted component evaluated to nothing");
}

/// The window draws with the second row of tabs up, with an integration
/// selected, and on a component's own tab.
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

/// An undo in the root puts its own tree back and nothing else: the
/// components it places stay as they are now, however they were when the step
/// was taken.
#[test]
pub(crate) fn an_undo_keeps_the_components_as_they_are_now() {
    let (mut app, group) = app_with_group("components-undo-current");
    app.run(Command::MakeComponent);
    let component = app.scene.component_of(group).unwrap();
    app.select_only(group);
    app.run(Command::Duplicate);
    app.activate_component(component);
    let root = app.scene.root();
    app.edit("Add", None);
    let sphere = app.scene.add_primitive("sphere", root, 2).unwrap();
    app.scene.get_mut(sphere).unwrap().position = Vec3::new(0.0, 0.0, 200.0);
    app.activate_component(ROOT_COMPONENT);

    app.run(Command::Undo);
    app.reevaluate_for_test();
    assert!(bounds(&app).1.z > 150.0, "the undo brought back the component as it was before the sphere");
    app.run(Command::Redo);
    app.reevaluate_for_test();
    assert!(bounds(&app).1.z > 150.0, "the redo brought back the component as it was before the sphere");

    // And an undo that puts back an integration of a deleted component shows
    // it as gone, not as it last looked.
    app.ask_delete_component(component);
    app.confirm_component_ask();
    app.run(Command::Undo);
    app.reevaluate_for_test();
    assert!(!app.evaluated.errors.is_empty(), "the integration of a deleted component still evaluated");
}

/// Undoing the placement of a saved primitive that carried a component inside
/// it takes both away, without asking; redo brings both back whole.
#[test]
pub(crate) fn undoing_a_placed_primitive_takes_the_components_inside_it_too() {
    let (mut app, group) = app_with_group("components-primitive-nested");
    app.run(Command::MakeComponent);
    app.select_only(group);
    app.save_selection_as_primitive();
    app.confirm_save_primitive();
    let entry = app.library[0].clone();
    app.run(Command::New);
    app.add_library_entry(&entry);
    assert_eq!(app.project.components.len(), 3, "the placed primitive did not bring its component along");
    app.reevaluate_for_test();
    let placed = bounds(&app);

    app.run(Command::Undo);
    assert_eq!(app.modal, Modal::None, "an untouched primitive asked before it was taken away");
    assert_eq!(app.project.components.len(), 1, "the undo left components behind");

    app.run(Command::Redo);
    assert_eq!(app.project.components.len(), 3);
    app.reevaluate_for_test();
    assert!(app.evaluated.errors.is_empty(), "{:?}", app.evaluated.errors);
    assert!(close(placed, bounds(&app)), "the redo did not bring the primitive back whole");
}

/// A paste into another project brings the components in, and undoing it
/// takes them out again.
#[test]
pub(crate) fn undoing_a_paste_from_another_project_takes_its_components_away() {
    let (mut app, group) = app_with_group("components-paste-undo");
    app.run(Command::MakeComponent);
    app.select_only(group);
    app.run(Command::Copy);
    app.run(Command::New);
    app.run(Command::Paste);
    assert!(app.project.uses_components());
    app.run(Command::Undo);
    assert!(!app.project.uses_components(), "the undone paste left its component in the project");
}

/// A new component made with a group selected is made of that group: the group
/// goes into it, an integration is left where it stood, and the component's
/// tab opens beside the root while the view stays on the integration.
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
