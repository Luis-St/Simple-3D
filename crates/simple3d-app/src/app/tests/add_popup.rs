//! Setting a shape up before it is added, from its palette tile.

use super::*;
use simple3d_core::config::Placement;
use simple3d_core::primitive::ParamValue;

/// Set a parameter of the shape being set up, as its field in the popup does.
fn set_width(app: &mut App, id: simple3d_core::scene::NodeId, width: f64) {
    app.edit("Set Width", None);
    crate::panel_properties::set_param(app, id, "width", ParamValue::Length(width));
}

#[test]
pub(crate) fn adding_from_the_popup_keeps_its_settings_as_one_undo_step() {
    let mut app = headless_app();
    let before = app.scene.depth_first().len();
    app.open_add_popup("box");
    let id = app.add_popup.as_ref().map(|adding| adding.id).expect("the popup did not open");
    assert_eq!(app.scene.depth_first().len(), before + 1, "the shape is not previewed in the scene");
    set_width(&mut app, id, 42.0);
    app.confirm_add_popup();
    assert!(app.add_popup.is_none());
    assert_eq!(params(&app, id).get("width"), Some(&ParamValue::Length(42.0)));
    app.undo_now();
    assert!(!app.scene.contains(id), "one undo did not take the set up shape back out");
}

#[test]
pub(crate) fn cancelling_the_popup_takes_the_shape_out_again() {
    let mut app = headless_app();
    let before = app.scene.depth_first().len();
    app.open_add_popup("box");
    let id = app.add_popup.as_ref().unwrap().id;
    set_width(&mut app, id, 42.0);
    app.cancel_add_popup();
    assert_eq!(app.scene.depth_first().len(), before, "Cancel left the shape in the document");
}

#[test]
pub(crate) fn deselecting_the_shape_being_set_up_cancels_it() {
    // A click into the viewport's empty space clears the selection.
    let mut app = headless_app();
    let before = app.scene.depth_first().len();
    app.open_add_popup("box");
    app.selection.clear();
    crate::add_popup::show(&mut app, &egui::Context::default());
    assert!(app.add_popup.is_none(), "the popup stayed open");
    assert_eq!(app.scene.depth_first().len(), before, "the shape stayed in the document");
}

#[test]
pub(crate) fn the_popup_chooses_where_its_shape_lands_without_changing_the_documents_choice() {
    let mut app = headless_app();
    app.cursor = Some(simple3d_geom::Vec3::new(30.0, 10.0, 0.0));
    app.open_add_popup("box");
    let id = app.add_popup.as_ref().unwrap().id;
    assert_eq!(app.scene.node(id).position, simple3d_geom::Vec3::ZERO);
    app.land_added(Placement::Cursor);
    assert_eq!(app.scene.node(id).position, simple3d_geom::Vec3::new(30.0, 10.0, 0.0));
    assert_eq!(app.settings.placement, Placement::Origin, "the document's placement was changed");
    app.land_added(Placement::OnSurface);
    app.confirm_add_popup();
    assert_eq!(app.placing.as_ref().map(|placing| placing.id), Some(id), "it does not follow the pointer after Add");
}

#[test]
pub(crate) fn holding_the_settings_key_while_adding_opens_the_popup() {
    let mut app = headless_app();
    app.add_primitive("box", true);
    assert!(app.add_popup.is_some(), "the held key did not open the popup");
    app.confirm_add_popup();
    app.add_primitive("box", false);
    assert!(app.add_popup.is_none(), "a plain add opened the popup");
}

#[test]
pub(crate) fn a_shapes_shortcut_pressed_with_the_settings_key_held_finds_its_shape() {
    // Shift+B is not B's binding, so the shape is looked up with the hold key's modifier taken off.
    let mut app = headless_app();
    app.keymap
        .bind(
            &simple3d_core::keymap::Action::Primitive("cylinder".into()),
            simple3d_core::keymap::Chord::key("B"),
            true,
        )
        .unwrap();
    let found = app.primitive_under_settings_key("B", |key| key == "B", egui::Modifiers::SHIFT);
    assert_eq!(found, Some("cylinder"));
    assert_eq!(app.primitive_under_settings_key("N", |key| key == "N", egui::Modifiers::SHIFT), None);
}

#[test]
pub(crate) fn the_shape_being_set_up_is_left_out_of_the_model_until_it_is_added() {
    // It is drawn as a template, in the tools' translucent orange, from its own mesh.
    let mut app = headless_app();
    app.open_add_popup("box");
    let id = app.adding().expect("the popup did not open");
    let evaluated = app.scene_without_adding().expect("no scene without it");
    assert!(!evaluated.node(id).adds(), "the shape being set up is still part of the model");
    assert!(app.scene.node(id).adds(), "the document itself lost the shape");
    app.confirm_add_popup();
    assert!(app.scene_without_adding().is_none());
}
