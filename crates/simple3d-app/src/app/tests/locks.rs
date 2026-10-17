//! A sphere's or cylinder's locked dimensions, unlocked and locked again from the lock toggle.

use super::*;
use crate::panel_properties::{apply_lock, is_locked, set_param, toggle_lock};
use simple3d_core::primitive::ParamValue;
use simple3d_core::scene::GroupOp;

/// Change one diameter as its field does: write it, then bring a locked group along.
fn set_diameter(app: &mut App, id: simple3d_core::scene::NodeId, key: &str, value: f64) {
    app.edit("Set diameter", None);
    set_param(app, id, key, ParamValue::Length(value));
    apply_lock(app, id, 1, key, ParamValue::Length(value));
}

#[test]
pub(crate) fn an_unlocked_group_with_equal_values_can_be_edited_one_value_at_a_time() {
    // Regression: the lock was read from the values alone, so unlocking a sphere whose diameters
    // were all 20 changed nothing, and the next edit still carried all three.
    let mut app = headless_app();
    app.add_node(Some("sphere"), GroupOp::Union);
    let id = app.primary().unwrap();
    assert!(is_locked(&app, id, 1), "a new sphere's diameters are not locked");
    toggle_lock(&mut app, id, 1, "diameter_x");
    assert!(!is_locked(&app, id, 1), "clicking the lock did not unlock it");
    set_diameter(&mut app, id, "diameter_x", 30.0);
    assert_eq!(params(&app, id).get("diameter_y"), Some(&ParamValue::Length(20.0)), "Y followed X while unlocked");

    toggle_lock(&mut app, id, 1, "diameter_x");
    assert!(is_locked(&app, id, 1), "clicking again did not lock it");
    assert_eq!(params(&app, id).get("diameter_z"), Some(&ParamValue::Length(30.0)), "locking did not equalise");
    set_diameter(&mut app, id, "diameter_y", 12.0);
    assert_eq!(params(&app, id).get("diameter_x"), Some(&ParamValue::Length(12.0)), "X did not follow once locked");
}
