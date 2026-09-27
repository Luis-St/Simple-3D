//! Shapes saved to the user's own library.

use super::*;
use simple3d_core::config::Placement;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_group_saved_as_a_primitive_comes_back_into_a_fresh_document() {
    // The library is per user, so what one document saves is on the next one's palette.
    let dir = temp_config_dir("library");
    let mut app = app_in(dir.clone());
    let root = app.scene.root();
    let plate = app.scene.add_primitive("plate", root, 0).unwrap();
    app.select_only(plate);
    app.run(Command::Group);
    let group = app.primary().unwrap();
    if let Some(node) = app.scene.get_mut(group) {
        node.name = "Bracket".into();
    }

    app.save_selection_as_primitive();
    assert_eq!(app.modal, Modal::SavePrimitive, "saving did not ask for a name");
    assert_eq!(app.primitive_name, "Bracket", "the name was not offered from the node");
    app.confirm_save_primitive();
    assert_eq!(app.modal, Modal::None);
    assert_eq!(app.library.len(), 1, "the palette did not pick the new entry up");

    // A second app on the same config directory, like reopening the program.
    let mut fresh = app_in(dir);
    assert_eq!(fresh.library.len(), 1);
    let entry = fresh.library[0].clone();
    assert_eq!(entry.name, "Bracket");

    fresh.settings.placement = Placement::Origin;
    fresh.add_library_entry(&entry);
    let added = fresh.primary().expect("nothing was added");
    assert_eq!(fresh.scene.node(added).name, "Bracket", "it arrived under some other name");
    // Placed as a component (issue 113): one node here, the saved group inside.
    assert!(fresh.scene.node(added).is_component(), "it did not arrive as a component");
    let component = fresh.scene.component_of(added).unwrap();
    let inside = fresh.component_scene(component).expect("the component is not in the project");
    assert_eq!(inside.node(inside.root()).children.len(), 1, "the group arrived without its child");
    assert_eq!(fresh.scene.node(added).position, Vec3::ZERO);

    // And it can be removed from the palette.
    fresh.delete_library_entry(&entry);
    assert!(fresh.library.is_empty());
}

#[test]
pub(crate) fn a_saved_primitive_lands_at_the_placement_rather_than_where_it_was_saved_from() {
    let dir = temp_config_dir("library-placement");
    let mut app = app_in(dir.clone());
    let root = app.scene.root();
    let plate = app.scene.add_primitive("plate", root, 0).unwrap();
    app.scene.get_mut(plate).unwrap().position = Vec3::new(70.0, 0.0, 0.0);
    app.select_only(plate);
    app.save_selection_as_primitive();
    app.confirm_save_primitive();

    let mut fresh = app_in(dir);
    fresh.settings.placement = Placement::Cursor;
    fresh.cursor = Some(Vec3::new(-5.0, 8.0, 0.0));
    let entry = fresh.library[0].clone();
    fresh.add_library_entry(&entry);
    assert_eq!(fresh.scene.node(fresh.primary().unwrap()).position, Vec3::new(-5.0, 8.0, 0.0));
}
