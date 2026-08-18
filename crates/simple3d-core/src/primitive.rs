//! The declarative primitive registry (spec section 3.2, "Extensibility").
//!
//! Adding a primitive means adding one `PrimitiveSpec` to `REGISTRY` and
//! nothing else. The Add menu, the property editor, the project file format,
//! the clipboard and undo all derive themselves from these declarations -- there
//! is deliberately no per-primitive user-interface code anywhere in the app.

use serde::{Deserialize, Serialize};
use simple3d_geom::{primitives as gen, Mesh};
use std::collections::BTreeMap;
use std::f64::consts::PI;

/// What a parameter is, which is all the property editor needs to render and
/// validate a field for it.
#[derive(Clone, Copy, Debug)]
pub enum ParamKind {
    /// A length, stored in millimetres and shown in the display unit.
    Length {
        min: f64,
    },
    /// An integer count, e.g. a polygon's number of sides.
    Count {
        min: u32,
        max: u32,
    },
    /// An angle in degrees.
    Angle {
        min: f64,
        max: f64,
    },
    Bool,
    /// A radio-style choice between named alternatives, for measurements that
    /// are otherwise ambiguous (across corners vs across flats, wall thickness
    /// vs inner diameter).
    Choice {
        options: &'static [&'static str],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum ParamValue {
    Length(f64),
    Count(u32),
    Angle(f64),
    Bool(bool),
    Choice(u32),
}

impl ParamValue {
    pub fn as_f64(self) -> f64 {
        match self {
            ParamValue::Length(v) | ParamValue::Angle(v) => v,
            ParamValue::Count(v) | ParamValue::Choice(v) => v as f64,
            ParamValue::Bool(b) => b as u8 as f64,
        }
    }

    pub fn as_u32(self) -> u32 {
        match self {
            ParamValue::Count(v) | ParamValue::Choice(v) => v,
            ParamValue::Bool(b) => b as u32,
            ParamValue::Length(v) | ParamValue::Angle(v) => v.max(0.0) as u32,
        }
    }

    pub fn as_bool(self) -> bool {
        match self {
            ParamValue::Bool(b) => b,
            other => other.as_u32() != 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParamSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: ParamKind,
    pub default: ParamValue,
    /// Parameters sharing a non-zero lock group can be tied together by a lock
    /// toggle in the property editor (a sphere's three diameters, a cylinder's
    /// two). Zero means no lock.
    pub lock_group: u8,
    /// When set, this parameter is only shown while the named `Choice`
    /// parameter has the given value -- how "wall thickness *or* inner
    /// diameter" is expressed without a second primitive type.
    pub shown_when: Option<(&'static str, u32)>,
}

impl ParamSpec {
    const fn length(key: &'static str, label: &'static str, default: f64) -> ParamSpec {
        ParamSpec {
            key,
            label,
            kind: ParamKind::Length { min: 0.0 },
            default: ParamValue::Length(default),
            lock_group: 0,
            shown_when: None,
        }
    }

    const fn locked_length(key: &'static str, label: &'static str, default: f64, group: u8) -> ParamSpec {
        ParamSpec { lock_group: group, ..ParamSpec::length(key, label, default) }
    }

    const fn positive_length(key: &'static str, label: &'static str, default: f64) -> ParamSpec {
        ParamSpec { kind: ParamKind::Length { min: 1e-3 }, ..ParamSpec::length(key, label, default) }
    }

    const fn sides(default: u32) -> ParamSpec {
        ParamSpec {
            key: "sides",
            label: "Number of sides",
            kind: ParamKind::Count { min: 3, max: 128 },
            default: ParamValue::Count(default),
            lock_group: 0,
            shown_when: None,
        }
    }

    const fn choice(key: &'static str, label: &'static str, options: &'static [&'static str]) -> ParamSpec {
        ParamSpec {
            key,
            label,
            kind: ParamKind::Choice { options },
            default: ParamValue::Choice(0),
            lock_group: 0,
            shown_when: None,
        }
    }

    const fn when(self, param: &'static str, value: u32) -> ParamSpec {
        ParamSpec { shown_when: Some((param, value)), ..self }
    }

    const fn count(key: &'static str, label: &'static str, default: u32, min: u32, max: u32) -> ParamSpec {
        ParamSpec {
            key,
            label,
            kind: ParamKind::Count { min, max },
            default: ParamValue::Count(default),
            lock_group: 0,
            shown_when: None,
        }
    }
}

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

/// How one bounding-box axis of a primitive relates to one of its parameters,
/// so a resize handle can write the real dimension instead of a scale factor
/// (spec section 6.2). `extent = value * factor`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisDriver {
    pub param: &'static str,
    pub factor: f64,
}

impl AxisDriver {
    const fn direct(param: &'static str) -> AxisDriver {
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

const BOXES: &str = "Boxes and prisms";
const ROUND: &str = "Round solids";
const CONES: &str = "Cones and pyramids";
const POLY: &str = "Regular polyhedra";
const FLAT: &str = "Flat shapes";

const MEASURE: &[&str] = &["Across corners", "Across flats"];
const WALL_MODE: &[&str] = &["Wall thickness", "Inner diameter"];
const SIZE_MODE: &[&str] = &["Circumscribed diameter", "Edge length"];

/// Inner diameter of a tube/ring, honouring the wall-thickness / inner-diameter
/// choice. Clamped so a wall thicker than the radius degenerates to a solid
/// rather than producing inverted geometry.
fn tube_inner(p: &Params) -> f64 {
    let outer = p.num("outer_diameter");
    let inner = if p.int("wall_mode") == 0 { outer - 2.0 * p.num("wall_thickness") } else { p.num("inner_diameter") };
    inner.clamp(0.0, outer)
}

/// Extent-to-diameter factor for a regular n-gon generated with a vertex at
/// angle 0: X spans the across-corners distance, so an across-flats diameter
/// has to be divided by cos(pi/n) to get it.
fn polygon_x_factor(p: &Params) -> f64 {
    let sides = p.int("sides").max(3);
    if p.int("measure") == 1 {
        1.0 / (PI / sides as f64).cos()
    } else {
        1.0
    }
}

pub static REGISTRY: &[PrimitiveSpec] = &[
    // -- Boxes and prisms ---------------------------------------------------
    PrimitiveSpec {
        type_id: "box",
        label: "Box",
        category: BOXES,
        params: &[
            ParamSpec::positive_length("width", "Width (X)", 20.0),
            ParamSpec::positive_length("depth", "Depth (Y)", 20.0),
            ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ],
        segmented: false,
        build: |p, _seg| gen::box_mesh(p.num("width"), p.num("depth"), p.num("height")),
        axes: |_p| {
            [Some(AxisDriver::direct("width")), Some(AxisDriver::direct("depth")), Some(AxisDriver::direct("height"))]
        },
    },
    PrimitiveSpec {
        type_id: "rounded_box",
        label: "Rounded box",
        category: BOXES,
        params: &[
            ParamSpec::positive_length("width", "Width (X)", 20.0),
            ParamSpec::positive_length("depth", "Depth (Y)", 20.0),
            ParamSpec::positive_length("height", "Height (Z)", 20.0),
            ParamSpec::length("corner_radius", "Corner radius", 3.0),
            ParamSpec::count("corner_segments", "Corner segments", 6, 1, 64),
        ],
        segmented: false,
        build: |p, _seg| {
            gen::rounded_box_mesh(
                p.num("width"),
                p.num("depth"),
                p.num("height"),
                p.num("corner_radius"),
                p.int("corner_segments"),
            )
        },
        axes: |_p| {
            [Some(AxisDriver::direct("width")), Some(AxisDriver::direct("depth")), Some(AxisDriver::direct("height"))]
        },
    },
    PrimitiveSpec {
        type_id: "wedge",
        label: "Wedge",
        category: BOXES,
        params: &[
            ParamSpec::positive_length("width", "Width (X)", 20.0),
            ParamSpec::positive_length("depth", "Depth (Y)", 20.0),
            ParamSpec::positive_length("height", "Height (Z)", 20.0),
            ParamSpec::length("top_width", "Top width (0 = sharp ridge)", 0.0),
        ],
        segmented: false,
        build: |p, _seg| gen::wedge_mesh(p.num("width"), p.num("depth"), p.num("height"), p.num("top_width")),
        axes: |_p| {
            [Some(AxisDriver::direct("width")), Some(AxisDriver::direct("depth")), Some(AxisDriver::direct("height"))]
        },
    },
    PrimitiveSpec {
        type_id: "prism",
        label: "Regular prism",
        category: BOXES,
        params: &[
            ParamSpec::sides(6),
            ParamSpec::positive_length("diameter", "Diameter", 20.0),
            ParamSpec::positive_length("height", "Height (Z)", 20.0),
            ParamSpec::choice("measure", "Diameter measured", MEASURE),
        ],
        segmented: false,
        build: |p, _seg| {
            gen::regular_prism_mesh(p.int("sides"), p.num("diameter"), p.num("height"), p.int("measure") == 1)
        },
        axes: |p| {
            [
                Some(AxisDriver { param: "diameter", factor: polygon_x_factor(p) }),
                // The Y extent of an n-gon with a vertex at angle 0 is neither
                // the across-corners nor the across-flats diameter for general
                // n, so no handle rather than a misleading one.
                None,
                Some(AxisDriver::direct("height")),
            ]
        },
    },
    // -- Round solids -------------------------------------------------------
    PrimitiveSpec {
        type_id: "sphere",
        label: "Sphere",
        category: ROUND,
        params: &[
            ParamSpec::locked_length("diameter_x", "Diameter X", 20.0, 1),
            ParamSpec::locked_length("diameter_y", "Diameter Y", 20.0, 1),
            ParamSpec::locked_length("diameter_z", "Diameter Z", 20.0, 1),
        ],
        segmented: true,
        build: |p, seg| gen::ellipsoid_mesh(p.num("diameter_x"), p.num("diameter_y"), p.num("diameter_z"), seg),
        axes: |_p| {
            [
                Some(AxisDriver::direct("diameter_x")),
                Some(AxisDriver::direct("diameter_y")),
                Some(AxisDriver::direct("diameter_z")),
            ]
        },
    },
    PrimitiveSpec {
        type_id: "spherical_cap",
        label: "Spherical cap",
        category: ROUND,
        params: &[
            ParamSpec::positive_length("diameter", "Diameter", 20.0),
            ParamSpec::positive_length("cap_height", "Cap height", 10.0),
        ],
        segmented: true,
        build: |p, seg| gen::spherical_cap_mesh(p.num("diameter"), p.num("cap_height"), seg),
        axes: |p| {
            // A cap shallower than a hemisphere is widest at its rim, and the
            // rim diameter depends on both parameters at once -- no single one
            // governs the X/Y extent, so no handle there.
            let (diameter, cap) = (p.num("diameter"), p.num("cap_height"));
            let d = (cap >= diameter / 2.0).then(|| AxisDriver::direct("diameter"));
            // The generator clamps the cap height to the sphere's diameter, so
            // beyond that the Z handle would stop tracking too.
            let z = (cap <= diameter).then(|| AxisDriver::direct("cap_height"));
            [d, d, z]
        },
    },
    PrimitiveSpec {
        type_id: "cylinder",
        label: "Cylinder",
        category: ROUND,
        params: &[
            ParamSpec::locked_length("diameter_x", "Diameter X", 20.0, 1),
            ParamSpec::locked_length("diameter_y", "Diameter Y", 20.0, 1),
            ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ],
        segmented: true,
        build: |p, seg| gen::cylinder_mesh(p.num("diameter_x"), p.num("diameter_y"), p.num("height"), seg),
        axes: |_p| {
            [
                Some(AxisDriver::direct("diameter_x")),
                Some(AxisDriver::direct("diameter_y")),
                Some(AxisDriver::direct("height")),
            ]
        },
    },
    PrimitiveSpec {
        type_id: "tube",
        label: "Tube",
        category: ROUND,
        params: &[
            ParamSpec::positive_length("outer_diameter", "Outer diameter", 20.0),
            ParamSpec::choice("wall_mode", "Wall given as", WALL_MODE),
            ParamSpec::positive_length("wall_thickness", "Wall thickness", 2.0).when("wall_mode", 0),
            ParamSpec::positive_length("inner_diameter", "Inner diameter", 16.0).when("wall_mode", 1),
            ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ],
        segmented: true,
        build: |p, seg| gen::tube_mesh(p.num("outer_diameter"), tube_inner(p), p.num("height"), seg),
        axes: |_p| {
            [
                Some(AxisDriver::direct("outer_diameter")),
                Some(AxisDriver::direct("outer_diameter")),
                Some(AxisDriver::direct("height")),
            ]
        },
    },
    PrimitiveSpec {
        type_id: "capsule",
        label: "Capsule",
        category: ROUND,
        params: &[
            ParamSpec::positive_length("diameter", "Diameter", 10.0),
            ParamSpec::positive_length("length", "Total length (with caps)", 30.0),
        ],
        segmented: true,
        build: |p, seg| gen::capsule_mesh(p.num("diameter"), p.num("length"), seg),
        axes: |p| {
            // A capsule shorter than its own diameter is a sphere: the length
            // no longer governs the Z extent, the diameter does.
            let long_enough = p.num("length") >= p.num("diameter");
            [
                Some(AxisDriver::direct("diameter")),
                Some(AxisDriver::direct("diameter")),
                Some(if long_enough { AxisDriver::direct("length") } else { AxisDriver::direct("diameter") }),
            ]
        },
    },
    PrimitiveSpec {
        type_id: "torus",
        label: "Torus",
        category: ROUND,
        params: &[
            ParamSpec::positive_length("ring_diameter", "Ring diameter (centre-line)", 30.0),
            ParamSpec::positive_length("tube_diameter", "Tube diameter", 6.0),
            ParamSpec {
                key: "sweep",
                label: "Sweep angle",
                kind: ParamKind::Angle { min: 1.0, max: 360.0 },
                default: ParamValue::Angle(360.0),
                lock_group: 0,
                shown_when: None,
            },
        ],
        segmented: true,
        build: |p, seg| gen::torus_mesh(p.num("ring_diameter"), p.num("tube_diameter"), p.num("sweep"), seg),
        // The X and Y extents are ring + tube diameter together, so neither
        // parameter alone governs them; only the Z handle is offered.
        axes: |_p| [None, None, Some(AxisDriver::direct("tube_diameter"))],
    },
    // -- Cones and pyramids -------------------------------------------------
    PrimitiveSpec {
        type_id: "cone",
        label: "Cone",
        category: CONES,
        params: &[
            ParamSpec::positive_length("bottom_diameter", "Bottom diameter", 20.0),
            ParamSpec::length("top_diameter", "Top diameter (0 = point)", 0.0),
            ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ],
        segmented: true,
        build: |p, seg| gen::cone_mesh(p.num("bottom_diameter"), p.num("top_diameter"), p.num("height"), seg),
        axes: |p| {
            // Whichever end is wider sets the X/Y extent.
            let wider =
                if p.num("top_diameter") > p.num("bottom_diameter") { "top_diameter" } else { "bottom_diameter" };
            [Some(AxisDriver::direct(wider)), Some(AxisDriver::direct(wider)), Some(AxisDriver::direct("height"))]
        },
    },
    PrimitiveSpec {
        type_id: "pyramid",
        label: "Pyramid",
        category: CONES,
        params: &[
            ParamSpec::positive_length("base_width", "Base width (X)", 20.0),
            ParamSpec::positive_length("base_depth", "Base depth (Y)", 20.0),
            ParamSpec::length("top_width", "Top width (0 = apex)", 0.0),
            ParamSpec::length("top_depth", "Top depth (0 = apex)", 0.0),
            ParamSpec::positive_length("height", "Height (Z)", 20.0),
        ],
        segmented: false,
        build: |p, _seg| {
            gen::pyramid_mesh(
                p.num("base_width"),
                p.num("base_depth"),
                p.num("top_width"),
                p.num("top_depth"),
                p.num("height"),
            )
        },
        axes: |p| {
            let wx = if p.num("top_width") > p.num("base_width") { "top_width" } else { "base_width" };
            let wy = if p.num("top_depth") > p.num("base_depth") { "top_depth" } else { "base_depth" };
            [Some(AxisDriver::direct(wx)), Some(AxisDriver::direct(wy)), Some(AxisDriver::direct("height"))]
        },
    },
    PrimitiveSpec {
        type_id: "regular_pyramid",
        label: "Regular pyramid",
        category: CONES,
        params: &[
            ParamSpec::sides(6),
            ParamSpec::positive_length("base_diameter", "Base diameter", 20.0),
            ParamSpec::length("top_diameter", "Top diameter (0 = apex)", 0.0),
            ParamSpec::positive_length("height", "Height (Z)", 20.0),
            ParamSpec::choice("measure", "Diameter measured", MEASURE),
        ],
        segmented: false,
        build: |p, _seg| {
            gen::regular_pyramid_mesh(
                p.int("sides"),
                p.num("base_diameter"),
                p.num("top_diameter"),
                p.num("height"),
                p.int("measure") == 1,
            )
        },
        axes: |p| {
            let wider = if p.num("top_diameter") > p.num("base_diameter") { "top_diameter" } else { "base_diameter" };
            [Some(AxisDriver { param: wider, factor: polygon_x_factor(p) }), None, Some(AxisDriver::direct("height"))]
        },
    },
    // -- Regular polyhedra --------------------------------------------------
    PrimitiveSpec {
        type_id: "tetrahedron",
        label: "Tetrahedron",
        category: POLY,
        params: &[
            ParamSpec::positive_length("size", "Size", 20.0),
            ParamSpec::choice("size_mode", "Size measured as", SIZE_MODE),
        ],
        segmented: false,
        build: |p, _seg| gen::tetrahedron_mesh(p.num("size"), p.int("size_mode") == 1),
        axes: |_p| [None, None, None],
    },
    PrimitiveSpec {
        type_id: "octahedron",
        label: "Octahedron",
        category: POLY,
        params: &[
            ParamSpec::positive_length("size", "Size", 20.0),
            ParamSpec::choice("size_mode", "Size measured as", SIZE_MODE),
        ],
        segmented: false,
        build: |p, _seg| gen::octahedron_mesh(p.num("size"), p.int("size_mode") == 1),
        axes: |_p| [None, None, None],
    },
    PrimitiveSpec {
        type_id: "dodecahedron",
        label: "Dodecahedron",
        category: POLY,
        params: &[
            ParamSpec::positive_length("size", "Size", 20.0),
            ParamSpec::choice("size_mode", "Size measured as", SIZE_MODE),
        ],
        segmented: false,
        build: |p, _seg| gen::dodecahedron_mesh(p.num("size"), p.int("size_mode") == 1),
        axes: |_p| [None, None, None],
    },
    PrimitiveSpec {
        type_id: "icosahedron",
        label: "Icosahedron",
        category: POLY,
        params: &[
            ParamSpec::positive_length("size", "Size", 20.0),
            ParamSpec::choice("size_mode", "Size measured as", SIZE_MODE),
        ],
        segmented: false,
        build: |p, _seg| gen::icosahedron_mesh(p.num("size"), p.int("size_mode") == 1),
        axes: |_p| [None, None, None],
    },
    // -- Flat shapes --------------------------------------------------------
    // Conveniences over the general forms. They call the same generators, so
    // they produce byte-identical geometry, as the spec requires.
    PrimitiveSpec {
        type_id: "plate",
        label: "Plate",
        category: FLAT,
        params: &[
            ParamSpec::positive_length("width", "Width (X)", 40.0),
            ParamSpec::positive_length("depth", "Depth (Y)", 20.0),
            ParamSpec::positive_length("thickness", "Thickness (Z)", 4.0),
        ],
        segmented: false,
        build: |p, _seg| gen::plate_mesh(p.num("width"), p.num("depth"), p.num("thickness")),
        axes: |_p| {
            [
                Some(AxisDriver::direct("width")),
                Some(AxisDriver::direct("depth")),
                Some(AxisDriver::direct("thickness")),
            ]
        },
    },
    PrimitiveSpec {
        type_id: "disc",
        label: "Disc",
        category: FLAT,
        params: &[
            ParamSpec::locked_length("diameter_x", "Diameter X", 20.0, 1),
            ParamSpec::locked_length("diameter_y", "Diameter Y", 20.0, 1),
            ParamSpec::positive_length("thickness", "Thickness (Z)", 2.0),
        ],
        segmented: true,
        build: |p, seg| gen::disc_mesh(p.num("diameter_x"), p.num("diameter_y"), p.num("thickness"), seg),
        axes: |_p| {
            [
                Some(AxisDriver::direct("diameter_x")),
                Some(AxisDriver::direct("diameter_y")),
                Some(AxisDriver::direct("thickness")),
            ]
        },
    },
    PrimitiveSpec {
        type_id: "ring",
        label: "Ring",
        category: FLAT,
        params: &[
            ParamSpec::positive_length("outer_diameter", "Outer diameter", 20.0),
            ParamSpec::choice("wall_mode", "Wall given as", WALL_MODE),
            ParamSpec::positive_length("wall_thickness", "Wall thickness", 2.0).when("wall_mode", 0),
            ParamSpec::positive_length("inner_diameter", "Inner diameter", 16.0).when("wall_mode", 1),
            ParamSpec::positive_length("thickness", "Thickness (Z)", 2.0),
        ],
        segmented: true,
        build: |p, seg| gen::ring_mesh(p.num("outer_diameter"), tube_inner(p), p.num("thickness"), seg),
        axes: |_p| {
            [
                Some(AxisDriver::direct("outer_diameter")),
                Some(AxisDriver::direct("outer_diameter")),
                Some(AxisDriver::direct("thickness")),
            ]
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_primitive_builds_a_manifold_mesh_from_its_defaults() {
        for spec in REGISTRY {
            let params = spec.default_params();
            let mesh = (spec.build)(&params, 32);
            assert!(mesh.triangle_count() > 0, "{}: empty mesh", spec.type_id);
            assert!(mesh.manifold_issue().is_none(), "{}: {}", spec.type_id, mesh.manifold_issue().unwrap());
        }
    }

    #[test]
    fn declared_axis_drivers_match_the_real_bounding_extent() {
        // A resize handle must write a parameter that genuinely governs the
        // extent it is dragging (spec section 6.2), so the declaration and the
        // generated mesh have to agree.
        for spec in REGISTRY {
            let params = spec.default_params();
            let mesh = (spec.build)(&params, 64);
            let (lo, hi) = mesh.bounds().unwrap();
            let size = [hi.x - lo.x, hi.y - lo.y, hi.z - lo.z];
            for (axis, driver) in (spec.axes)(&params).iter().enumerate() {
                let Some(driver) = driver else { continue };
                let expected = params.num(driver.param) * driver.factor;
                assert!(
                    (expected - size[axis]).abs() < 1e-6,
                    "{}: axis {axis} driver {} predicts {expected}, mesh measures {}",
                    spec.type_id,
                    driver.param,
                    size[axis]
                );
            }
        }
    }

    #[test]
    fn axis_drivers_stay_truthful_after_a_parameter_changes() {
        // The invariant a resize handle relies on is that `axes(params)`
        // predicts the real bounding extents for *any* parameter values, not
        // just the defaults -- otherwise a handle would keep tracking the
        // cursor after the parameter it writes stopped governing that extent.
        // Where that can happen (a spherical cap dragged wider than twice its
        // cap height is widest at its rim, not at its equator) the declaration
        // has to withdraw the handle, and this test is what proves it does.
        for spec in REGISTRY {
            let base = spec.default_params();
            for p in spec.params {
                if !matches!(p.kind, ParamKind::Length { .. }) {
                    continue;
                }
                for scale in [0.25, 0.5, 1.5, 3.0] {
                    let mut edited = base.clone();
                    let value = (base.num(p.key) * scale).max(1e-3);
                    edited.insert(p.key.to_string(), ParamValue::Length(value));
                    let (lo, hi) = (spec.build)(&edited, 64).bounds().unwrap();
                    let size = [hi.x - lo.x, hi.y - lo.y, hi.z - lo.z];
                    for (axis, driver) in (spec.axes)(&edited).iter().enumerate() {
                        let Some(driver) = driver else { continue };
                        let predicted = edited.num(driver.param) * driver.factor;
                        assert!(
                            (predicted - size[axis]).abs() < 1e-6,
                            "{}: with {}={value}, axis {axis} driver {} predicts {predicted} but the mesh measures {}",
                            spec.type_id,
                            p.key,
                            driver.param,
                            size[axis]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn flat_shape_aliases_match_their_general_forms() {
        // Spec section 3.2: "must produce identical geometry to their equivalents".
        let plate = lookup("plate").unwrap();
        let boxy = lookup("box").unwrap();
        let mut bp = boxy.default_params();
        bp.insert("width".into(), ParamValue::Length(40.0));
        bp.insert("depth".into(), ParamValue::Length(20.0));
        bp.insert("height".into(), ParamValue::Length(4.0));
        let a = (plate.build)(&plate.default_params(), 32);
        let b = (boxy.build)(&bp, 32);
        assert_eq!(a.indices, b.indices);
        assert_eq!(a.positions, b.positions);

        let disc = lookup("disc").unwrap();
        let cyl = lookup("cylinder").unwrap();
        let mut cp = cyl.default_params();
        cp.insert("height".into(), ParamValue::Length(2.0));
        let a = (disc.build)(&disc.default_params(), 32);
        let b = (cyl.build)(&cp, 32);
        assert_eq!(a.positions, b.positions);
    }

    #[test]
    fn tube_wall_modes_agree() {
        let tube = lookup("tube").unwrap();
        let mut by_wall = tube.default_params();
        by_wall.insert("wall_mode".into(), ParamValue::Choice(0));
        by_wall.insert("wall_thickness".into(), ParamValue::Length(2.0));
        let mut by_inner = tube.default_params();
        by_inner.insert("wall_mode".into(), ParamValue::Choice(1));
        by_inner.insert("inner_diameter".into(), ParamValue::Length(16.0));
        assert_eq!(tube_inner(&by_wall), tube_inner(&by_inner));
        let a = (tube.build)(&by_wall, 32);
        let b = (tube.build)(&by_inner, 32);
        assert_eq!(a.positions, b.positions);
    }

    #[test]
    fn migration_fills_new_parameters_and_drops_unknown_ones() {
        let spec = lookup("box").unwrap();
        let mut stored: Params = Params::new();
        stored.insert("width".into(), ParamValue::Length(55.0));
        stored.insert("obsolete".into(), ParamValue::Length(1.0));
        let migrated = spec.migrate_params(&stored);
        assert_eq!(migrated.num("width"), 55.0);
        assert_eq!(migrated.num("depth"), 20.0);
        assert!(!migrated.contains_key("obsolete"));
        assert_eq!(migrated.len(), spec.params.len());
    }

    #[test]
    fn parameter_keys_are_unique_within_a_type() {
        for spec in REGISTRY {
            for (i, p) in spec.params.iter().enumerate() {
                assert!(
                    !spec.params[..i].iter().any(|q| q.key == p.key),
                    "{}: duplicate parameter key {}",
                    spec.type_id,
                    p.key
                );
            }
            if let Some((dep, _)) = spec.params.iter().find_map(|p| p.shown_when) {
                assert!(spec.param(dep).is_some(), "{}: shown_when names unknown {dep}", spec.type_id);
            }
        }
    }

    #[test]
    fn type_ids_are_unique() {
        for (i, spec) in REGISTRY.iter().enumerate() {
            assert!(REGISTRY[..i].iter().all(|s| s.type_id != spec.type_id), "duplicate {}", spec.type_id);
        }
    }
}
