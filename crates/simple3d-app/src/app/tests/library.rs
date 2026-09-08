//! Shapes saved to the user's own library.

use super::*;
use simple3d_core::config::Placement;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_group_saved_as_a_primitive_comes_back_into_a_fresh_document() {
    // The library is per user, not per project: what is saved out of one
    // document is on the palette of the next one.
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

    // A second application on the same config directory -- which is what
    // opening the program again is -- has it on the palette.
    let mut fresh = app_in(dir);
    assert_eq!(fresh.library.len(), 1);
    let entry = fresh.library[0].clone();
    assert_eq!(entry.name, "Bracket");

    fresh.settings.placement = Placement::Origin;
    fresh.add_library_entry(&entry);
    let added = fresh.primary().expect("nothing was added");
    assert_eq!(fresh.scene.node(added).name, "Bracket", "it arrived under some other name");
    assert!(fresh.scene.node(added).is_group());
    assert_eq!(fresh.scene.node(added).children.len(), 1, "the group arrived without its child");
    assert_eq!(fresh.scene.node(added).position, Vec3::ZERO);

    // And it can be taken off the palette again.
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
