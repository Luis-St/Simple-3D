//! The definition of one shape: its parameters, the mesh it builds, and
//! which parameter each axis handle drives.

use super::*;
pub use registry::REGISTRY;
use simple3d_geom::Mesh;

/// How one bounding-box axis of a primitive relates to one of its parameters,
/// so a resize handle can write the real dimension instead of a scale factor
/// (spec section 6.2). `extent = value * factor`.
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
    /// Whether the scene's default segment count (and a per-node override)
    /// applies to this type.
    pub segmented: bool,
    pub build: fn(&Params, u32) -> Mesh,
    /// Which parameter governs each of the X, Y and Z bounding extents. `None`
    /// on an axis means no resize handle is offered there, rather than a handle
    /// that silently does nothing.
    pub axes: fn(&Params) -> [Option<AxisDriver>; 3],
}

impl PrimitiveSpec {
    /// Default parameter values for a freshly added node.
    pub fn default_params(&self) -> Params {
        self.params.iter().map(|p| (p.key.to_string(), p.default)).collect()
    }

    /// Fill in any parameter the given map is missing and drop any it does not
    /// recognise. Used when loading an older project file, so a primitive that
    /// gained a parameter migrates silently (spec section 10).
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

    /// Whether a parameter should be shown, given the current values of the
    /// choice parameters it depends on.
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
