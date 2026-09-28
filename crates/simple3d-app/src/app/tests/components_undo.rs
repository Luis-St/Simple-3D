//! Components under undo and redo (issue 113).

use super::components::{app_with_group, bounds};
use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::scene::ROOT_COMPONENT;
use simple3d_geom::Vec3;

/// Undo removes a new component and redo restores it, without asking while nothing is lost.
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

/// Undoing a component that has been worked on asks first.
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

/// An undo in the root restores only its own tree, not the components it places.
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

    // An integration of a deleted component comes back as gone.
    app.ask_delete_component(component);
    app.confirm_component_ask();
    app.run(Command::Undo);
    app.reevaluate_for_test();
    assert!(!app.evaluated.errors.is_empty(), "the integration of a deleted component still evaluated");
}
