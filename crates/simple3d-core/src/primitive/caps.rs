//! How far a parameter can go before its generator stops following it (issue 131). Generators hold
//! a corner radius to the box, a hole to the nut and so on, so a field left unbounded kept growing
//! while the shape stood still; the fields stop where the shape does instead.

use super::registry::{gear_spec, sprocket_spec};
use super::*;

impl PrimitiveSpec {
    /// The largest value `key` changes the shape at, given the other parameters; `None` if the
    /// generator takes any value. Lengths in millimetres.
    pub fn cap(&self, key: &str, p: &Params) -> Option<f64> {
        let half = |key: &str| p.num(key) / 2.0;
        let cap = match (self.type_id, key) {
            ("rounded_box", "corner_radius") => {
                let across = half("width").min(half("depth"));
                match (p.int("corner_style"), p.int("round_edges")) {
                    (0, 1) => across.min(half("height")),
                    (0, 2) => across.min(p.num("height")),
                    _ => across,
                }
            }
            ("chamfered_box", "chamfer") => {
                let across = half("width").max(half("depth"));
                if p.int("edges") == 1 {
                    across
                } else {
                    across.max(half("height"))
                }
            }
            ("chamfered_box", "chamfer_x") => half("width"),
            ("chamfered_box", "chamfer_y") => half("depth"),
            ("chamfered_box", "chamfer_z") => half("height"),
            // A wall reaching the middle leaves it solid.
            ("prism", "wall") => {
                let flat = (std::f64::consts::PI / p.int("sides").max(3) as f64).cos();
                let corner = if p.int("measure") == 1 { half("diameter") / flat } else { half("diameter") };
                corner * flat
            }
            ("sphere", "wall") => half("diameter_x").min(half("diameter_y")).min(half("diameter_z")),
            ("spherical_cap", "cap_height") if p.int("size_mode") == 0 => p.num("diameter"),
            ("spherical_cap", "wall") => match p.int("size_mode") {
                1 => cap_sphere_radius(p),
                _ => half("diameter"),
            },
            ("tube" | "ring", "wall_thickness") => half("outer_diameter"),
            ("tube" | "ring", "inner_diameter") => p.num("outer_diameter") * 0.99,
            ("tube", "top_inner_diameter") => p.num("top_outer_diameter"),
            ("torus", "inner_diameter") => p.num("outer_diameter") * 0.999,
            ("cone", "wall") => {
                let (bottom, top) = (half("bottom_diameter"), half("top_diameter"));
                let slope = (top - bottom) / p.num("height").max(1e-12);
                bottom.max(top) / (1.0 + slope * slope).sqrt()
            }
            (
                "plate",
                "corner_radius" | "radius_front_right" | "radius_back_right" | "radius_back_left" | "radius_front_left",
            ) => half("width").min(half("depth")),
            ("slot", "wall") => p.num("length").min(p.num("width")) / 2.0,
            ("slot", "edge_rounding") => {
                let side = p.num("length").min(p.num("width")) / 2.0;
                let wall = p.num("wall");
                let edge = side.min(half("thickness"));
                if wall > 1e-9 && wall < side {
                    edge.min(wall)
                } else {
                    edge
                }
            }
            ("hex_nut" | "hex_standoff", "hole_diameter") => {
                let corners = if p.int("measure") == 1 { 1.154_700_538_379_251_5 } else { 1.0 };
                p.num("across_flats") / corners * 0.95
            }
            ("screw_hole", "recess") => p.num("length"),
            ("threaded_rod", "thread_depth") => {
                let slope = (p.num("thread_angle").clamp(0.0, 179.0) / 2.0).to_radians().tan();
                return (slope > 0.0).then(|| p.num("pitch") / 2.0 / slope);
            }
            ("spring", "wire") => p.num("diameter") * 0.9,
            ("sprocket", "roller") => p.num("pitch") * 0.8,
            ("spur_gear", _) => {
                let spec = gear_spec(p);
                return body_cap(key, &spec.body(), spec.root_radius());
            }
            ("sprocket", _) => {
                let spec = sprocket_spec(p);
                return body_cap(key, &spec.body, spec.root_radius());
            }
            _ => return None,
        };
        Some(cap.max(0.0))
    }

    /// Bring every parameter down to its cap, after `p` changed: a thinner box takes its corner radius
    /// with it, so the field shows what is built.
    pub fn hold_to_caps(&self, p: &mut Params) {
        for spec in self.params {
            // A hidden one builds nothing, so it keeps its number for when it is chosen again.
            if !matches!(spec.kind, ParamKind::Length { .. }) || !self.param_visible(spec, p) {
                continue;
            }
            let Some(cap) = self.cap(spec.key, p) else { continue };
            if p.num(spec.key) > cap {
                p.insert(spec.key.to_string(), ParamValue::Length(cap));
            }
        }
    }
}

/// The radius of the sphere a cap given by its base is cut from.
fn cap_sphere_radius(p: &Params) -> f64 {
    simple3d_geom::primitives::cap_sphere_diameter(p.num("base_diameter"), p.num("cap_height")) / 2.0
}

/// A gear body's bore, hub and keyway, held inside teeth whose roots are `root` out.
fn body_cap(key: &str, body: &simple3d_geom::primitives::Body, root: f64) -> Option<f64> {
    let hole = body.bore_radius(root);
    // Without a bore there is no keyway to hold, and its numbers wait for one.
    if key.starts_with("key_") && hole <= 1e-6 {
        return None;
    }
    let cap = match key {
        "bore" => root * 2.0 * 0.9,
        "hub_diameter" => root * 2.0 * 0.98,
        "key_width" => hole * 2.0 * 0.95,
        "key_depth" => root * 0.95 - hole,
        _ => return None,
    };
    Some(cap.max(0.0))
}
