//! Keeping the turned profile as a feature node, as one undo step.

use super::*;
use crate::app::{App, Status};
use simple3d_core::primitive::{ParamValue, Params};
use simple3d_core::scene::{feature_params, Feature, Placing, Profile};

/// Add the solid the popup is showing. Out of the profile's front it adds; into the object the
/// profile lies on it cuts; a sketch on nothing adds either way.
pub(crate) fn commit(app: &mut App) {
    app.revolve.refresh();
    let tool = &app.revolve;
    let (Some(target), Some(axis)) = (tool.profile.clone(), tool.axis.clone()) else {
        app.status = Status::Info(step_hint(tool).into());
        return;
    };
    let angle = tool.signed_angle();
    if let Some((_, Err(why))) = &tool.preview {
        app.status = Status::Warning(why.clone());
        return;
    }
    let cut = tool.cuts();
    let placing = if cut { Placing::Cut } else { Placing::Add };
    let (profile, sketch) = match &target {
        Target::Face(hit) => (Profile::Face { outline: hit.captured.outline.clone() }, None),
        Target::Region(hit, _) => {
            (Profile::Sketch { sketch: (*hit.sketch).clone(), regions: vec![hit.pick.clone()] }, Some(hit.node))
        }
    };
    let mut params = feature_params(true, &Params::new());
    params.insert("sweep".into(), ParamValue::Angle(angle.abs().clamp(1.0, 360.0)));
    params.insert("flip".into(), ParamValue::Bool(angle < 0.0));
    let feature = Feature { profile, axis: Some(axis.stored.clone()) };
    let name = if cut { "Revolve cut" } else { "Revolve" };
    let owner = target.owner().map(|owner| (owner, placing));
    let frame = target.frame();
    if app.add_feature_node("Revolve", name, feature, params, &frame, owner, sketch).is_none() {
        app.status = Status::Warning("That profile could not be revolved there".into());
        return;
    }
    app.revolve.clear();
    app.status = Status::Info(format!(
        "{} by {} degrees; pick another profile to revolve it",
        if cut { "Cut a revolve" } else { "Revolved" },
        simple3d_core::unit::format_angle(angle.abs())
    ));
}
