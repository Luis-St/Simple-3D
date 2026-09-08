//! A shape's parameters as a whole, and reading a value out by name.

use super::*;
use std::collections::BTreeMap;

/// A primitive's parameter values, keyed by `ParamSpec::key`. `BTreeMap` so the
/// project file's key order is stable and diffable.
pub type Params = BTreeMap<String, ParamValue>;

pub trait ParamsExt {
    fn num(&self, key: &str) -> f64;
    fn int(&self, key: &str) -> u32;
    fn flag(&self, key: &str) -> bool;
}

impl ParamsExt for Params {
    fn num(&self, key: &str) -> f64 {
        self.get(key).copied().map(ParamValue::as_f64).unwrap_or(0.0)
    }
    fn int(&self, key: &str) -> u32 {
        self.get(key).copied().map(ParamValue::as_u32).unwrap_or(0)
    }
    fn flag(&self, key: &str) -> bool {
        self.get(key).copied().map(ParamValue::as_bool).unwrap_or(false)
    }
}
