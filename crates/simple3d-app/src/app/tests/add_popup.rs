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
