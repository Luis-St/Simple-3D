mod custom;
mod grips;
mod kinds;
mod migration;
mod noise;
mod variations;
mod vary;

use super::*;
use crate::primitive::{ParamValue, Params};
use simple3d_geom::Vec3;

fn with(overrides: &[(&str, ParamValue)]) -> Params {
    let mut params = default_params();
    for (key, value) in overrides {
        params.insert((*key).to_string(), *value);
    }
    params
}

fn labels(params: &Params) -> Vec<&'static str> {
    grips(params).into_iter().map(|g| g.label).collect()
}

fn near(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-9
}

/// A rule of the given stages, and nothing else.
fn rule(stages: &[Stage]) -> Params {
    let mut params = with(&[("kind", ParamValue::Choice(CUSTOM))]);
    for (index, stage) in stages.iter().enumerate() {
        set_stage(&mut params, index, stage);
    }
    params.insert("stages".to_string(), ParamValue::Count(stages.len() as u32));
    params
}
