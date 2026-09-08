mod custom;
mod grips;
mod kinds;

use super::*;
use crate::primitive::{ParamValue, Params};

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
