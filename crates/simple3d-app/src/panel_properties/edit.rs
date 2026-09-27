//! Writing a value back, and the axis locks that decide what a resize carries.

use crate::app::{App, Status};
use simple3d_core::primitive::{ParamValue, ParamsExt};
use simple3d_core::scene::NodeId;

pub(crate) fn set_param(app: &mut App, id: NodeId, key: &str, value: ParamValue) {
    if let Some(params) = app.scene.get_mut(id).and_then(|n| n.params_mut()) {
        params.insert(key.to_string(), value);
    }
}

/// Whether a lock group's parameters hold the same value; the lock is derived from the numbers, so
/// files hold no hidden state.
pub(crate) fn is_locked(app: &App, id: NodeId, group: u8) -> bool {
    let Some(spec) = app.scene.node(id).spec() else { return false };
    let Some(params) = app.scene.node(id).params() else { return false };
    let keys: Vec<&str> = spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key).collect();
    match keys.split_first() {
        Some((first, rest)) => {
            let reference = params.num(first);
            rest.iter().all(|k| (params.num(k) - reference).abs() < 1e-9)
        }
        None => false,
    }
}

/// Clicking the lock equalises the group to the clicked field, or if locked, just reports unlocking.
pub(crate) fn toggle_lock(app: &mut App, id: NodeId, group: u8, key: &str) {
    let Some(spec) = app.scene.node(id).spec() else { return };
    let keys: Vec<String> = spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key.to_string()).collect();
    if is_locked(app, id, group) {
        // Nothing to change; the fields are already independent in the model.
        app.status = Status::Info("Unlocked; edit the diameters independently".into());
        return;
    }
    let Some(value) = app.scene.node(id).params().map(|p| p.num(key)) else { return };
    app.edit("Lock dimensions", None);
    for other in keys {
        set_param(app, id, &other, ParamValue::Length(value));
    }
}

/// When a locked group's member changes, bring the others with it.
pub(crate) fn apply_lock(app: &mut App, id: NodeId, group: u8, key: &str, value: ParamValue) {
    if group == 0 {
        return;
    }
    // Judge the lock from before this edit, since the group no longer agrees now.
    let Some(spec) = app.scene.node(id).spec() else { return };
    let keys: Vec<String> = spec.params.iter().filter(|p| p.lock_group == group).map(|p| p.key.to_string()).collect();
    let others: Vec<&String> = keys.iter().filter(|k| k.as_str() != key).collect();
    let Some(params) = app.scene.node(id).params() else { return };
    // Locked before means every other member still agrees.
    let was_locked = match others.split_first() {
        Some((first, rest)) => {
            let reference = params.num(first);
            rest.iter().all(|k| (params.num(k) - reference).abs() < 1e-9)
        }
        None => false,
    };
    if !was_locked {
        return;
    }
    let others: Vec<String> = others.into_iter().cloned().collect();
    for other in others {
        set_param(app, id, &other, value);
    }
    app.fields.clear();
}

/// Switching a tube between wall thickness and inner diameter carries the geometry across, so the
/// shape does not jump.
pub(crate) fn sync_wall_mode(app: &mut App, id: NodeId, key: &str, chosen: u32) {
    if key != "wall_mode" {
        return;
    }
    let Some(params) = app.scene.node(id).params().cloned() else { return };
    let outer = params.num("outer_diameter");
    if chosen == 1 {
        let inner = (outer - 2.0 * params.num("wall_thickness")).clamp(0.0, outer);
        set_param(app, id, "inner_diameter", ParamValue::Length(inner));
    } else {
        let wall = ((outer - params.num("inner_diameter")) / 2.0).max(0.0);
        set_param(app, id, "wall_thickness", ParamValue::Length(wall));
    }
    app.fields.clear();
}
