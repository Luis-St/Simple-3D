//! Pattern nodes (issue 67): a node that repeats its children under a set of
//! transforms rather than as manual duplicates.
//!
//! A pattern holds children the way a group does, but it evaluates to copies of
//! them laid out by a rule -- a line, a grid, a ring, a mirror, a helix or a
//! spiral. Editing the original edits every copy, and the count is a number
//! rather than a hundred pasted nodes to keep in step.
//!
//! Its parameters ride in the same [`Params`] map a primitive uses, so the
//! property editor renders and validates them with no code of its own, the
//! project file and the clipboard carry them already, and undo covers them. The
//! kind is the first parameter -- a choice -- and every other parameter is shown
//! only for the kind it belongs to, exactly as a primitive hides "wall
//! thickness" behind its own choice.

use crate::primitive::{ParamKind, ParamSpec, ParamValue, Params, ParamsExt};
use crate::xform::Xform;
use simple3d_geom::Vec3;

/// The kinds of pattern, in the order they appear in the "kind" choice; the
/// index into this list is the value the choice parameter holds.
pub const KINDS: &[&str] = &["Linear", "Grid", "Circular", "Mirror", "Helix", "Spiral"];

const LINEAR: u32 = 0;
const GRID: u32 = 1;
const CIRCULAR: u32 = 2;
const MIRROR: u32 = 3;
const HELIX: u32 = 4;
const SPIRAL: u32 = 5;

const fn length(key: &'static str, label: &'static str, default: f64, when: (&'static str, u32)) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Length { min: f64::NEG_INFINITY },
        default: ParamValue::Length(default),
        lock_group: 0,
        shown_when: Some(when),
    }
}

const fn count(key: &'static str, label: &'static str, default: u32, when: (&'static str, u32)) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Count { min: 1, max: 512 },
        default: ParamValue::Count(default),
        lock_group: 0,
        shown_when: Some(when),
    }
}

const fn angle(key: &'static str, label: &'static str, default: f64, when: (&'static str, u32)) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Angle { min: -360.0, max: 360.0 },
        default: ParamValue::Angle(default),
        lock_group: 0,
        shown_when: Some(when),
    }
}

/// The axis a turning pattern turns about, and whose perpendicular a radius is
/// measured in.
const AXES: &[&str] = &["X", "Y", "Z"];

const fn axis(key: &'static str, when: (&'static str, u32)) -> ParamSpec {
    ParamSpec {
        key,
        label: "Axis",
        kind: ParamKind::Choice { options: AXES },
        // Z by default: a ring or a helix stands up, which is what a build plate
        // wants.
        default: ParamValue::Choice(2),
        lock_group: 0,
        shown_when: Some(when),
    }
}

/// Every parameter a pattern can carry, across all kinds. The kind choice comes
/// first and gates the rest, so the property editor shows exactly the fields the
/// current kind uses.
pub const PARAMS: &[ParamSpec] = &[
    ParamSpec {
        key: "kind",
        label: "Kind",
        kind: ParamKind::Choice { options: KINDS },
        default: ParamValue::Choice(LINEAR),
        lock_group: 0,
        shown_when: None,
    },
    // Linear: a run of copies along a fixed step.
    count("count", "Copies", 3, ("kind", LINEAR)),
    length("step_x", "Step X", 20.0, ("kind", LINEAR)),
    length("step_y", "Step Y", 0.0, ("kind", LINEAR)),
    length("step_z", "Step Z", 0.0, ("kind", LINEAR)),
    // Grid: a lattice, one count and one step per axis.
    count("grid_x", "Columns (X)", 3, ("kind", GRID)),
    count("grid_y", "Rows (Y)", 2, ("kind", GRID)),
    count("grid_z", "Layers (Z)", 1, ("kind", GRID)),
    length("grid_step_x", "Step X", 20.0, ("kind", GRID)),
    length("grid_step_y", "Step Y", 20.0, ("kind", GRID)),
    length("grid_step_z", "Step Z", 20.0, ("kind", GRID)),
    // Circular: copies evenly round a turn.
    count("circ_count", "Copies", 6, ("kind", CIRCULAR)),
    angle("circ_span", "Span", 360.0, ("kind", CIRCULAR)),
    length("circ_radius", "Radius", 0.0, ("kind", CIRCULAR)),
    axis("circ_axis", ("kind", CIRCULAR)),
    // Mirror: the original and its reflection across a plane.
    axis("mirror_axis", ("kind", MIRROR)),
    // Helix: turn and rise together.
    count("helix_count", "Copies", 8, ("kind", HELIX)),
    angle("helix_angle", "Angle per copy", 45.0, ("kind", HELIX)),
    length("helix_rise", "Rise per copy", 5.0, ("kind", HELIX)),
    length("helix_radius", "Radius", 20.0, ("kind", HELIX)),
    axis("helix_axis", ("kind", HELIX)),
    // Spiral: turn while the radius grows.
    count("spiral_count", "Copies", 12, ("kind", SPIRAL)),
    angle("spiral_angle", "Angle per copy", 30.0, ("kind", SPIRAL)),
    length("spiral_radius", "Start radius", 10.0, ("kind", SPIRAL)),
    length("spiral_growth", "Radius per copy", 5.0, ("kind", SPIRAL)),
    length("spiral_rise", "Rise per copy", 0.0, ("kind", SPIRAL)),
    axis("spiral_axis", ("kind", SPIRAL)),
];

/// A fresh pattern's parameters.
pub fn default_params() -> Params {
    PARAMS.iter().map(|p| (p.key.to_string(), p.default)).collect()
}

/// Fill in anything a stored map is missing and drop anything it does not know,
/// so a pattern from an older file migrates the way a primitive does.
pub fn migrate_params(stored: &Params) -> Params {
    PARAMS
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

/// Whether a parameter should be shown, given the kind currently chosen. The
/// same rule a primitive's choice-gated parameters follow.
pub fn param_visible(spec: &ParamSpec, values: &Params) -> bool {
    match spec.shown_when {
        None => true,
        Some((key, want)) => values.int(key) == want,
    }
}

/// A single copy the pattern makes: where to put it, and whether it is a
/// reflection (whose winding must be flipped so its faces still point out).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    pub xform: Xform,
    pub mirrored: bool,
}

impl Instance {
    fn plain(xform: Xform) -> Instance {
        Instance { xform, mirrored: false }
    }
}

/// Euler angles that turn `angle_deg` about axis 0, 1 or 2.
fn rotation_about(axis: usize, angle_deg: f64) -> Vec3 {
    let mut r = Vec3::ZERO;
    match axis {
        0 => r.x = angle_deg,
        1 => r.y = angle_deg,
        _ => r.z = angle_deg,
    }
    r
}

/// The unit vector along an axis.
fn unit(axis: usize) -> Vec3 {
    match axis {
        0 => Vec3::new(1.0, 0.0, 0.0),
        1 => Vec3::new(0.0, 1.0, 0.0),
        _ => Vec3::new(0.0, 0.0, 1.0),
    }
}

/// The axis a radius is measured along for a turn about `axis`: the next axis
/// round, so a ring about Z lies out along X.
fn radial_axis(axis: usize) -> usize {
    (axis + 1) % 3
}

/// The copies a pattern makes, in its own frame, ready to be laid over the
/// mesh of its children. Always at least one -- the original -- so a pattern
/// with a count of one, or of nonsense, still shows what it holds.
pub fn instances(params: &Params) -> Vec<Instance> {
    match params.int("kind") {
        GRID => grid(params),
        CIRCULAR => circular(params),
        MIRROR => mirror(params),
        HELIX => helix(params),
        SPIRAL => spiral(params),
        _ => linear(params),
    }
}

fn linear(params: &Params) -> Vec<Instance> {
    let count = params.int("count").max(1);
    let step = Vec3::new(params.num("step_x"), params.num("step_y"), params.num("step_z"));
    (0..count).map(|i| Instance::plain(Xform::from_translation(step * i as f64))).collect()
}

fn grid(params: &Params) -> Vec<Instance> {
    let (nx, ny, nz) = (params.int("grid_x").max(1), params.int("grid_y").max(1), params.int("grid_z").max(1));
    let step = Vec3::new(params.num("grid_step_x"), params.num("grid_step_y"), params.num("grid_step_z"));
    let mut out = Vec::new();
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let offset = Vec3::new(step.x * i as f64, step.y * j as f64, step.z * k as f64);
                out.push(Instance::plain(Xform::from_translation(offset)));
            }
        }
    }
    out
}

/// The angle between copies for a turn: a full turn divides by the count so the
/// first and last copy do not land on each other, a partial one divides by the
/// gaps so both ends are placed.
fn step_angle(span: f64, count: u32) -> f64 {
    if count <= 1 {
        return 0.0;
    }
    if (span.abs() - 360.0).abs() < 1e-6 {
        span / count as f64
    } else {
        span / (count - 1) as f64
    }
}

/// A copy at radius `r` and angle `a` about `ax`, optionally lifted along the
/// axis: place it out at the radius, turn it about the axis, then lift it.
fn turned(ax: usize, r: f64, angle_deg: f64, lift: f64) -> Xform {
    let place = Xform::from_translation(unit(radial_axis(ax)) * r);
    let turn = Xform::from_pos_rot(Vec3::ZERO, rotation_about(ax, angle_deg));
    let rise = Xform::from_translation(unit(ax) * lift);
    rise.compose(&turn.compose(&place))
}

fn circular(params: &Params) -> Vec<Instance> {
    let count = params.int("circ_count").max(1);
    let span = params.num("circ_span");
    let radius = params.num("circ_radius");
    let ax = params.int("circ_axis").min(2) as usize;
    let step = step_angle(span, count);
    (0..count).map(|i| Instance::plain(turned(ax, radius, step * i as f64, 0.0))).collect()
}

fn mirror(params: &Params) -> Vec<Instance> {
    let ax = params.int("mirror_axis").min(2) as usize;
    // A reflection across the plane through the origin whose normal is the axis:
    // the identity with that axis negated. Its winding is flipped when it is laid
    // down so the reflected faces still point outward.
    let mut m = Xform::IDENTITY;
    m.m[ax][ax] = -1.0;
    vec![Instance::plain(Xform::IDENTITY), Instance { xform: m, mirrored: true }]
}

fn helix(params: &Params) -> Vec<Instance> {
    let count = params.int("helix_count").max(1);
    let step = params.num("helix_angle");
    let rise = params.num("helix_rise");
    let radius = params.num("helix_radius");
    let ax = params.int("helix_axis").min(2) as usize;
    (0..count).map(|i| Instance::plain(turned(ax, radius, step * i as f64, rise * i as f64))).collect()
}

fn spiral(params: &Params) -> Vec<Instance> {
    let count = params.int("spiral_count").max(1);
    let step = params.num("spiral_angle");
    let r0 = params.num("spiral_radius");
    let dr = params.num("spiral_growth");
    let rise = params.num("spiral_rise");
    let ax = params.int("spiral_axis").min(2) as usize;
    (0..count).map(|i| Instance::plain(turned(ax, r0 + dr * i as f64, step * i as f64, rise * i as f64))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(overrides: &[(&str, ParamValue)]) -> Params {
        let mut params = default_params();
        for (key, value) in overrides {
            params.insert((*key).to_string(), *value);
        }
        params
    }

    #[test]
    fn a_linear_pattern_steps_its_copies_along_a_fixed_offset() {
        let params = with(&[
            ("kind", ParamValue::Choice(LINEAR)),
            ("count", ParamValue::Count(4)),
            ("step_x", ParamValue::Length(10.0)),
            ("step_y", ParamValue::Length(0.0)),
            ("step_z", ParamValue::Length(0.0)),
        ]);
        let copies = instances(&params);
        assert_eq!(copies.len(), 4);
        assert!((copies[0].xform.t - Vec3::ZERO).length() < 1e-9, "the first copy is the original");
        assert!((copies[3].xform.t - Vec3::new(30.0, 0.0, 0.0)).length() < 1e-9);
        assert!(copies.iter().all(|c| !c.mirrored));
    }

    #[test]
    fn a_grid_pattern_fills_a_lattice() {
        let params = with(&[
            ("kind", ParamValue::Choice(GRID)),
            ("grid_x", ParamValue::Count(3)),
            ("grid_y", ParamValue::Count(2)),
            ("grid_z", ParamValue::Count(1)),
            ("grid_step_x", ParamValue::Length(10.0)),
            ("grid_step_y", ParamValue::Length(5.0)),
        ]);
        let copies = instances(&params);
        assert_eq!(copies.len(), 6, "3 x 2 x 1");
        // The far corner of the lattice.
        assert!(copies.iter().any(|c| (c.xform.t - Vec3::new(20.0, 5.0, 0.0)).length() < 1e-9));
    }

    #[test]
    fn a_circular_pattern_spaces_a_full_turn_by_the_count() {
        let params = with(&[
            ("kind", ParamValue::Choice(CIRCULAR)),
            ("circ_count", ParamValue::Count(4)),
            ("circ_span", ParamValue::Angle(360.0)),
            ("circ_radius", ParamValue::Length(10.0)),
            ("circ_axis", ParamValue::Choice(2)),
        ]);
        let copies = instances(&params);
        assert_eq!(copies.len(), 4);
        // Radius out along +X (the radial axis for a turn about Z), and a quarter
        // turn brings the second copy round to +Y.
        assert!((copies[0].xform.point(Vec3::ZERO) - Vec3::new(10.0, 0.0, 0.0)).length() < 1e-6);
        assert!(
            (copies[1].xform.point(Vec3::ZERO) - Vec3::new(0.0, 10.0, 0.0)).length() < 1e-6,
            "{:?}",
            copies[1].xform.point(Vec3::ZERO)
        );
    }

    #[test]
    fn a_partial_circular_pattern_places_both_ends() {
        let params = with(&[
            ("kind", ParamValue::Choice(CIRCULAR)),
            ("circ_count", ParamValue::Count(3)),
            ("circ_span", ParamValue::Angle(90.0)),
            ("circ_radius", ParamValue::Length(10.0)),
            ("circ_axis", ParamValue::Choice(2)),
        ]);
        let copies = instances(&params);
        // Three copies over 90 degrees: at 0, 45 and 90.
        assert!((copies[2].xform.point(Vec3::ZERO) - Vec3::new(0.0, 10.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn a_mirror_pattern_reflects_and_marks_the_copy_for_a_winding_flip() {
        let params = with(&[("kind", ParamValue::Choice(MIRROR)), ("mirror_axis", ParamValue::Choice(0))]);
        let copies = instances(&params);
        assert_eq!(copies.len(), 2);
        assert!(!copies[0].mirrored, "the original is not a reflection");
        assert!(copies[1].mirrored, "the reflection must be flagged for a winding flip");
        // A point at +X reflects to -X across the X-normal plane.
        assert!((copies[1].xform.point(Vec3::new(3.0, 1.0, 2.0)) - Vec3::new(-3.0, 1.0, 2.0)).length() < 1e-9);
    }

    #[test]
    fn a_helix_turns_and_rises_together() {
        let params = with(&[
            ("kind", ParamValue::Choice(HELIX)),
            ("helix_count", ParamValue::Count(5)),
            ("helix_angle", ParamValue::Angle(90.0)),
            ("helix_rise", ParamValue::Length(4.0)),
            ("helix_radius", ParamValue::Length(10.0)),
            ("helix_axis", ParamValue::Choice(2)),
        ]);
        let copies = instances(&params);
        assert_eq!(copies.len(), 5);
        // The fourth copy: three 90-degree turns (back to +Y from +X twice round)
        // and three rises of 4.
        let p = copies[3].xform.point(Vec3::ZERO);
        assert!((p.z - 12.0).abs() < 1e-6, "it did not rise: {p:?}");
        // Radius held: the horizontal distance stays the radius.
        assert!(((p.x * p.x + p.y * p.y).sqrt() - 10.0).abs() < 1e-6, "the radius drifted: {p:?}");
    }

    #[test]
    fn a_spiral_grows_its_radius_each_copy() {
        let params = with(&[
            ("kind", ParamValue::Choice(SPIRAL)),
            ("spiral_count", ParamValue::Count(4)),
            ("spiral_angle", ParamValue::Angle(0.0)),
            ("spiral_radius", ParamValue::Length(5.0)),
            ("spiral_growth", ParamValue::Length(3.0)),
            ("spiral_axis", ParamValue::Choice(2)),
        ]);
        let copies = instances(&params);
        // No angle, so every copy is out along +X, at a growing radius.
        assert!((copies[0].xform.point(Vec3::ZERO) - Vec3::new(5.0, 0.0, 0.0)).length() < 1e-6);
        assert!((copies[3].xform.point(Vec3::ZERO) - Vec3::new(14.0, 0.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn every_parameter_belongs_to_a_real_kind_and_the_kinds_are_all_listed() {
        // A parameter gated on a kind index past the end of the list would never
        // show; the choice's own options are the list, so this cannot drift.
        for spec in PARAMS {
            if let Some((key, value)) = spec.shown_when {
                assert_eq!(key, "kind", "{}", spec.key);
                assert!((value as usize) < KINDS.len(), "{} is gated on a kind that does not exist", spec.key);
            }
        }
        // Every kind has at least the copies or axis it needs to differ from the
        // others -- a kind with no parameters of its own would be a dead choice.
        for k in 0..KINDS.len() as u32 {
            let mut params = default_params();
            params.insert("kind".into(), ParamValue::Choice(k));
            assert!(!instances(&params).is_empty(), "kind {k} produced no copies");
        }
    }
}
