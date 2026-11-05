//! A pushed sketch region kept as a feature node (issue 74), extruded the way it was dragged.

use crate::app::{App, Status};
use crate::sketch_nodes::RegionHit;
use simple3d_core::primitive::{ParamValue, Params};
use simple3d_core::scene::{feature_params, Feature, NodeId, Placing, Profile};
use simple3d_core::unit::format_length;

/// Extrude `hit` by `distance` along the sketch's normal: in, it cuts into `owner`, the object the
/// sketch lies on; with none, it runs out the other side instead.
pub(crate) fn commit(app: &mut App, hit: &RegionHit, owner: Option<NodeId>, distance: f64) {
    let cut = distance < 0.0 && owner.is_some();
    let placing = if cut { Placing::Cut } else { Placing::Add };
    let profile = Profile::Sketch { sketch: (*hit.sketch).clone(), regions: vec![hit.pick.clone()] };
    let mut params = feature_params(false, &Params::new());
    params.insert("distance".into(), ParamValue::Length(distance.abs().max(0.01)));
    params.insert("flip".into(), ParamValue::Bool(distance < 0.0));
    let feature = Feature { profile, axis: None };
    let name = if cut { "Cut" } else { "Extrusion" };
    let undo = if distance > 0.0 { "Push region" } else { "Pull region" };
    if app
        .add_feature_node(undo, name, feature, params, &hit.frame, owner.map(|o| (o, placing)), Some(hit.node))
        .is_none()
    {
        app.status = Status::Warning("That region could not be pushed or pulled".into());
        return;
    }
    let unit = app.unit();
    let sketch = app.scene.node(hit.node).name.clone();
    app.status = Status::Info(format!(
        "{} a region of {sketch} by {} {}",
        if cut { "Cut" } else { "Extruded" },
        format_length(distance.abs(), unit),
        unit.suffix()
    ));
}
