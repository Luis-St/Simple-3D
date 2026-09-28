//! Components saved, placed from saved primitives and pasted between projects (issue 113).

use super::components::{app_with_group, bounds, close};
use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::project;
use simple3d_core::scene::ROOT_COMPONENT;

/// A project with components round-trips whole; one without keeps the old file format.
#[test]
pub(crate) fn a_project_with_components_saves_and_opens_whole() {
    let dir = temp_config_dir("components-file");
    let (mut app, group) = app_with_group("components-file-app");
    let plain = dir.join("plain.simple3d");
    app.save_to(&plain);
    assert!(std::fs::read_to_string(&plain).unwrap().contains(&format!("\"format\": {}", project::PLAIN_FORMAT)));

    app.run(Command::MakeComponent);
    let component = app.scene.component_of(group).unwrap();
    // Saving from the component's own tab must not matter.
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

/// A saved primitive placed twice gives two unlinked components (issue 113).
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

    app.run(Command::Undo);
    assert!(app.project.get(second).is_none());
}

/// Pasting within a project reuses the component; into another project it brings a copy.
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

/// Undoing a saved primitive placement with a nested component removes both without asking;
/// redo restores both.
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

/// Pasting into another project brings the components in, and undo removes them.
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
