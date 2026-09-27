//! One shape's definition: its parameters, the mesh it builds, and which parameter each axis drives.

use super::*;
pub use registry::REGISTRY;
use simple3d_geom::Mesh;

/// How a bounding axis relates to a parameter (`extent = value * factor`), so resize writes the real
/// dimension (spec section 6.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisDriver {
    pub param: &'static str,
    pub factor: f64,
}

impl AxisDriver {
    pub(super) const fn direct(param: &'static str) -> AxisDriver {
        AxisDriver { param, factor: 1.0 }
    }
}

pub struct PrimitiveSpec {
    pub type_id: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    pub params: &'static [ParamSpec],
    /// Whether the scene's default segment count (and per-node override) applies.
    pub segmented: bool,
    pub build: fn(&Params, u32) -> Mesh,
    /// Which parameter governs each bounding extent; `None` means no resize handle on that axis.
    pub axes: fn(&Params) -> [Option<AxisDriver>; 3],
}

impl PrimitiveSpec {
    /// Default parameter values for a new node.
    pub fn default_params(&self) -> Params {
        self.params.iter().map(|p| (p.key.to_string(), p.default)).collect()
    }

    /// Fill missing parameters and drop unknown ones, so older files migrate silently (spec section 10).
    pub fn migrate_params(&self, stored: &Params) -> Params {
        self.params
            .iter()
            .map(|p| {
                let value = stored
                    .get(p.key)
                    .copied()
                    .filter(|v| std::mem::discriminant(v) == std::mem::discriminant(&p.default))
                    .unwrap_or(p.default);
                (p.key.to_string(), value)
            })
            .collect()
    }

    pub fn param(&self, key: &str) -> Option<&ParamSpec> {
        self.params.iter().find(|p| p.key == key)
    }

    /// Whether a parameter is shown, given the choice parameters it depends on.
    pub fn param_visible(&self, spec: &ParamSpec, values: &Params) -> bool {
        match spec.shown_when {
            None => true,
            Some((key, want)) => values.int(key) == want,
        }
    }
}

pub fn lookup(type_id: &str) -> Option<&'static PrimitiveSpec> {
    REGISTRY.iter().find(|s| s.type_id == type_id)
}

pub fn categories() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for spec in REGISTRY {
        if !out.contains(&spec.category) {
            out.push(spec.category);
        }
    }
    out
}
