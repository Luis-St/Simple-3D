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

/// The most copies one pattern will ever lay down.
///
/// Each count is clamped to 512 on its own, but a grid *multiplies* three of
/// them: 512 on every axis is 134 million copies, which is tens of gigabytes of
/// transforms before a single triangle is placed. The per-axis clamp cannot see
/// that, so the total is capped here as well -- generously enough for any run a
/// person lays out by hand, and low enough that a mistyped grid is a redrawn
/// preview rather than an out-of-memory kill.
pub const MAX_INSTANCES: usize = 4096;

/// A fresh pattern's parameters.
pub fn default_params() -> Params {
    PARAMS.iter().map(|p| (p.key.to_string(), p.default)).collect()
}

/// A fresh pattern's parameters, with every distance scaled to the shapes the
/// pattern is being wrapped around (issue 67).
///
/// A fixed default cannot be right for both a 2 mm pin and a 200 mm plate: the
/// stock 20 mm step is exactly the width of the default box, which lays the
/// copies down face to face -- one welded, non-manifold lump rather than three
/// boxes. Deriving the numbers from what is actually being repeated puts a
/// visible gap between the copies whatever their size, and gives a ring or a
/// helix a radius its own contents fit around.
pub fn params_for_size(size: Vec3) -> Params {
    let mut params = default_params();
    // Half the shape again, so a copy clears the one before it by half its own
    // width -- the spacing someone laying parts out by eye tends to reach for.
    let step = |extent: f64| ParamValue::Length(if extent > 1e-9 { extent * 1.5 } else { 20.0 });
    let across = size.x.max(size.y);
    let radius = if across > 1e-9 { across * 1.5 } else { 20.0 };
    for (key, value) in [
        ("step_x", step(size.x)),
        ("grid_step_x", step(size.x)),
        ("grid_step_y", step(size.y)),
        ("grid_step_z", step(size.z)),
        ("circ_radius", ParamValue::Length(radius)),
        ("helix_radius", ParamValue::Length(radius)),
        ("helix_rise", step(size.z)),
        ("spiral_radius", ParamValue::Length(radius)),
        ("spiral_growth", step(size.x)),
    ] {
        params.insert(key.to_string(), value);
    }
    params
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
pub fn radial_axis(axis: usize) -> usize {
    (axis + 1) % 3
}

/// The copies a pattern makes, in its own frame, ready to be laid over the
/// mesh of its children. Always at least one -- the original -- so a pattern
/// with a count of one, or of nonsense, still shows what it holds.
pub fn instances(params: &Params) -> Vec<Instance> {
    let mut out = match params.int("kind") {
        GRID => grid(params),
        CIRCULAR => circular(params),
        MIRROR => mirror(params),
        HELIX => helix(params),
        SPIRAL => spiral(params),
        _ => linear(params),
    };
    // The cap is applied here rather than in each kind so no kind can forget it,
    // and by truncation rather than by refusing: the pattern still shows what it
    // makes, just not more of it than anything can draw. `instance_count` says
    // whether this bit, so the editor can tell the user.
    out.truncate(MAX_INSTANCES);
    out
}

/// How many copies a pattern asks for and how many it will actually lay down.
/// The two differ only where [`MAX_INSTANCES`] has cut in, which is what the
/// property editor says out loud rather than silently drawing fewer.
pub fn instance_count(params: &Params) -> (usize, usize) {
    let wanted = match params.int("kind") {
        GRID => {
            let (nx, ny, nz) = (
                params.int("grid_x").max(1) as usize,
                params.int("grid_y").max(1) as usize,
                params.int("grid_z").max(1) as usize,
            );
            nx.saturating_mul(ny).saturating_mul(nz)
        }
        CIRCULAR => params.int("circ_count").max(1) as usize,
        MIRROR => 2,
        HELIX => params.int("helix_count").max(1) as usize,
        SPIRAL => params.int("spiral_count").max(1) as usize,
        _ => params.int("count").max(1) as usize,
    };
    (wanted, wanted.min(MAX_INSTANCES))
}

fn linear(params: &Params) -> Vec<Instance> {
    let count = params.int("count").max(1);
    let step = Vec3::new(params.num("step_x"), params.num("step_y"), params.num("step_z"));
    (0..count).map(|i| Instance::plain(Xform::from_translation(step * i as f64))).collect()
}

fn grid(params: &Params) -> Vec<Instance> {
    let (nx, ny, nz) = (params.int("grid_x").max(1), params.int("grid_y").max(1), params.int("grid_z").max(1));
    let step = Vec3::new(params.num("grid_step_x"), params.num("grid_step_y"), params.num("grid_step_z"));
    // Stop at the cap *while building* rather than by truncating afterwards: the
    // three counts multiply, so a full 512 x 512 x 512 would have to be
    // allocated -- 14 GB of transforms -- before anything could trim it.
    let mut out = Vec::new();
    'fill: for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                if out.len() >= MAX_INSTANCES {
                    break 'fill;
                }
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

// -- laying a pattern out in the viewport (issue 67) -------------------------
//
// A pattern is a rule, and a rule is a handful of numbers -- but nobody lays a
// ring of bolt holes out by typing a radius. Each kind therefore offers a set
// of *grips*: points in the pattern's own frame that can be taken hold of in
// the viewport and dragged, each one writing exactly one of the numbers the
// property editor shows. They live here, beside the maths that places the
// copies, so the handle a user drags and the copy it sits on cannot disagree,
// and so the whole layout can be tested without a viewport.

/// What dragging a grip writes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drive {
    /// A length. The distance the grip is dragged to, less `base` and divided
    /// by `per`, is the value -- so a grip that sits at the *last* copy divides
    /// by the number of gaps and writes the step between two.
    ///
    /// One key sets that parameter on its own; three set a run that can point
    /// anywhere, whose direction is kept and whose length becomes the value, so
    /// lengthening a run that steps diagonally keeps the diagonal rather than
    /// straightening it onto X.
    Length { keys: &'static [&'static str], base: f64, per: f64, min: f64 },
    /// A count: how many copies reach as far as the grip was dragged, at the
    /// spacing the pattern already has. This is what "lay one out" means for a
    /// straight run -- drag outward and copies follow the pointer.
    Count { key: &'static str, base: f64, per: f64 },
    /// An angle in degrees: the angle the grip was dragged round to, about
    /// `axis`. A grip that sits on the *second* copy divides by the one turn
    /// between it and the first, and so writes the turn per copy.
    Angle { key: &'static str, axis: usize, per: f64 },
}

/// One handle a pattern offers for laying itself out by eye.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grip {
    /// What it drives, for the tooltip and for the undo step it records. Unique
    /// within one kind, which is what identifies a grip across the frames of a
    /// drag -- an index would shift under the drag itself, since adding a copy
    /// can add a grip.
    pub label: &'static str,
    /// Where the grip sits, in the pattern's own frame.
    pub at: Vec3,
    /// The line it slides along: a point on that line and a unit direction. For
    /// an [`Drive::Angle`] grip the line is the axis it turns about, `from` is
    /// the centre of the turn, and `radius` is how far out the grip rides.
    pub from: Vec3,
    pub dir: Vec3,
    pub radius: f64,
    pub drive: Drive,
}

impl Grip {
    fn slide(label: &'static str, at: Vec3, dir: Vec3, drive: Drive) -> Grip {
        Grip { label, at, from: Vec3::ZERO, dir, radius: 0.0, drive }
    }
}

const LINEAR_RUN: &[&str] = &["step_x", "step_y", "step_z"];

/// A length that may not go below zero: a radius or the length of a run, both
/// of which mean nothing negative.
const POSITIVE: f64 = 0.0;
/// A length that may go either way: a step, a rise or a growth, each of which
/// lays the copies out backwards or downwards when it is negative.
const EITHER_WAY: f64 = f64::NEG_INFINITY;

/// The grips a pattern offers, in its own frame.
///
/// A grip whose home is the pattern's own origin is dropped: that is where the
/// move manipulator already sits, and two handles on one point cannot both be
/// grabbed. It is also where a run of no length puts everything, and a run with
/// no length has nothing to take hold of -- those numbers are typed once and
/// then dragged.
pub fn grips(params: &Params) -> Vec<Grip> {
    let mut out = match params.int("kind") {
        GRID => grid_grips(params),
        CIRCULAR => circular_grips(params),
        // A mirror is a plane and two copies. There is no distance and no count
        // to lay out, so it offers nothing to drag.
        MIRROR => Vec::new(),
        HELIX => helix_grips(params),
        SPIRAL => spiral_grips(params),
        _ => linear_grips(params),
    };
    out.retain(|g| g.at.length() > 1e-6);
    out
}

/// The grip with this label, for a drag that started on it a frame ago.
pub fn grip(params: &Params, label: &str) -> Option<Grip> {
    grips(params).into_iter().find(|g| g.label == label)
}

/// Write what a grip was dragged to.
///
/// `value` is a distance along the grip's own line, in the pattern's frame, or
/// an angle in degrees for a span. Nothing here rounds to the document's step:
/// that belongs to the caller, which is where the modifier keys are read.
pub fn apply_grip(params: &mut Params, grip: &Grip, value: f64) {
    match grip.drive {
        Drive::Length { keys, base, per, min } => {
            let per = if per.abs() > 1e-9 { per } else { 1.0 };
            let v = ((value - base) / per).max(min);
            if keys.len() == 3 {
                // A run that can point anywhere: its direction is kept and only
                // its length is written.
                let step = Vec3::new(params.num(keys[0]), params.num(keys[1]), params.num(keys[2]));
                let length = step.length();
                let dir = if length > 1e-9 { step * (1.0 / length) } else { unit(0) };
                let scaled = dir * v;
                for (axis, key) in keys.iter().enumerate() {
                    let component = match axis {
                        0 => scaled.x,
                        1 => scaled.y,
                        _ => scaled.z,
                    };
                    params.insert((*key).to_string(), ParamValue::Length(component));
                }
            } else {
                params.insert(keys[0].to_string(), ParamValue::Length(v));
            }
        }
        Drive::Count { key, base, per } => {
            let per = if per.abs() > 1e-9 { per } else { 1.0 };
            // The same 1..512 the count parameters carry, so a grip can never
            // write a number the property editor would refuse.
            let count = ((value - base) / per).round().clamp(1.0, 512.0) as u32;
            params.insert(key.to_string(), ParamValue::Count(count));
        }
        Drive::Angle { key, per, .. } => {
            let per = if per.abs() > 1e-9 { per } else { 1.0 };
            params.insert(key.to_string(), ParamValue::Angle((value / per).clamp(-360.0, 360.0)));
        }
    }
}

fn linear_grips(params: &Params) -> Vec<Grip> {
    let count = params.int("count").max(1);
    let step = Vec3::new(params.num("step_x"), params.num("step_y"), params.num("step_z"));
    let length = step.length();
    let dir = if length > 1e-9 { step * (1.0 / length) } else { unit(0) };
    let mut out = Vec::new();
    if count >= 2 {
        out.push(Grip::slide(
            "Spacing",
            step * (count - 1) as f64,
            dir,
            Drive::Length { keys: LINEAR_RUN, base: 0.0, per: (count - 1) as f64, min: POSITIVE },
        ));
    }
    if length > 1e-9 {
        // One step past the last copy: drag it out and the run grows a copy at
        // a time at the spacing already set.
        out.push(Grip::slide(
            "Copies",
            dir * (length * count as f64),
            dir,
            Drive::Count { key: "count", base: 0.0, per: length },
        ));
    }
    out
}

fn grid_grips(params: &Params) -> Vec<Grip> {
    const COUNT_KEYS: [&str; 3] = ["grid_x", "grid_y", "grid_z"];
    const STEP_KEYS: [&[&str]; 3] = [&["grid_step_x"], &["grid_step_y"], &["grid_step_z"]];
    const SPACING: [&str; 3] = ["Column spacing", "Row spacing", "Layer spacing"];
    const HOW_MANY: [&str; 3] = ["Columns", "Rows", "Layers"];
    let mut out = Vec::new();
    for axis in 0..3 {
        let count = params.int(COUNT_KEYS[axis]).max(1);
        let step = params.num(STEP_KEYS[axis][0]);
        let dir = unit(axis);
        if count >= 2 {
            out.push(Grip::slide(
                SPACING[axis],
                dir * (step * (count - 1) as f64),
                dir,
                Drive::Length { keys: STEP_KEYS[axis], base: 0.0, per: (count - 1) as f64, min: EITHER_WAY },
            ));
        }
        if step.abs() > 1e-9 {
            out.push(Grip::slide(
                HOW_MANY[axis],
                dir * (step * count as f64),
                dir,
                Drive::Count { key: COUNT_KEYS[axis], base: 0.0, per: step },
            ));
        }
    }
    out
}

fn circular_grips(params: &Params) -> Vec<Grip> {
    let ax = params.int("circ_axis").min(2) as usize;
    let radius = params.num("circ_radius");
    let span = params.num("circ_span");
    let radial = unit(radial_axis(ax));
    let mut out = vec![Grip::slide(
        "Radius",
        radial * radius,
        radial,
        Drive::Length { keys: &["circ_radius"], base: 0.0, per: 1.0, min: POSITIVE },
    )];
    if radius.abs() > 1e-9 {
        // The span grip rides on a wider circle than the copies do. On the ring
        // itself a full turn would end where it began, on top of the radius
        // grip, and neither could be picked out from the other.
        let ring = radius * 1.3;
        out.push(Grip {
            label: "Span",
            at: turned(ax, ring, span, 0.0).point(Vec3::ZERO),
            from: Vec3::ZERO,
            dir: unit(ax),
            radius: ring,
            drive: Drive::Angle { key: "circ_span", axis: ax, per: 1.0 },
        });
    }
    // A ring has no outward run to drag copies along -- its copies fill the
    // span evenly however many there are -- so the span grip is what lays it
    // out and the count stays a number.
    out
}

fn helix_grips(params: &Params) -> Vec<Grip> {
    let ax = params.int("helix_axis").min(2) as usize;
    let count = params.int("helix_count").max(1);
    let radius = params.num("helix_radius");
    let rise = params.num("helix_rise");
    let radial = unit(radial_axis(ax));
    let up = unit(ax);
    let mut out = vec![Grip::slide(
        "Radius",
        radial * radius,
        radial,
        Drive::Length { keys: &["helix_radius"], base: 0.0, per: 1.0, min: POSITIVE },
    )];
    // Both grips ride the axis rather than the helix itself: a grip on the last
    // copy would swing round the turn as the drag changed it, and chase the
    // pointer sideways while it was being pulled straight up.
    if count >= 2 {
        out.push(Grip::slide(
            "Rise",
            up * (rise * (count - 1) as f64),
            up,
            Drive::Length { keys: &["helix_rise"], base: 0.0, per: (count - 1) as f64, min: EITHER_WAY },
        ));
    }
    if rise.abs() > 1e-9 {
        out.push(Grip::slide(
            "Copies",
            up * (rise * count as f64),
            up,
            Drive::Count { key: "helix_count", base: 0.0, per: rise },
        ));
    }
    // The twist, taken hold of on the second copy: it is one turn from the
    // first, so the angle the grip is carried round to *is* the turn per copy,
    // and the rest of the helix follows behind it.
    if count >= 2 && radius.abs() > 1e-9 {
        let angle = params.num("helix_angle");
        out.push(Grip {
            label: "Turn per copy",
            at: turned(ax, radius, angle, rise).point(Vec3::ZERO),
            from: up * rise,
            dir: up,
            radius,
            drive: Drive::Angle { key: "helix_angle", axis: ax, per: 1.0 },
        });
    }
    out
}

fn spiral_grips(params: &Params) -> Vec<Grip> {
    let ax = params.int("spiral_axis").min(2) as usize;
    let count = params.int("spiral_count").max(1);
    let start = params.num("spiral_radius");
    let growth = params.num("spiral_growth");
    let rise = params.num("spiral_rise");
    let radial = unit(radial_axis(ax));
    let up = unit(ax);
    // Three grips along one radial line, at the first copy's radius, the last
    // one's, and one copy beyond: a ruler out from the centre that says where
    // the spiral starts, how fast it opens and how far it goes.
    let mut out = vec![Grip::slide(
        "Start radius",
        radial * start,
        radial,
        Drive::Length { keys: &["spiral_radius"], base: 0.0, per: 1.0, min: POSITIVE },
    )];
    if count >= 2 {
        out.push(Grip::slide(
            "Radius per copy",
            radial * (start + growth * (count - 1) as f64),
            radial,
            Drive::Length { keys: &["spiral_growth"], base: start, per: (count - 1) as f64, min: EITHER_WAY },
        ));
    }
    if growth.abs() > 1e-9 {
        out.push(Grip::slide(
            "Copies",
            radial * (start + growth * count as f64),
            radial,
            Drive::Count { key: "spiral_count", base: start, per: growth },
        ));
    }
    if count >= 2 && rise.abs() > 1e-9 {
        out.push(Grip::slide(
            "Rise",
            up * (rise * (count - 1) as f64),
            up,
            Drive::Length { keys: &["spiral_rise"], base: 0.0, per: (count - 1) as f64, min: EITHER_WAY },
        ));
    }
    let second = start + growth;
    if count >= 2 && second.abs() > 1e-9 {
        let angle = params.num("spiral_angle");
        out.push(Grip {
            label: "Turn per copy",
            at: turned(ax, second, angle, rise).point(Vec3::ZERO),
            from: up * rise,
            dir: up,
            radius: second,
            drive: Drive::Angle { key: "spiral_angle", axis: ax, per: 1.0 },
        });
    }
    out
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

    fn labels(params: &Params) -> Vec<&'static str> {
        grips(params).into_iter().map(|g| g.label).collect()
    }

    #[test]
    fn a_grip_sits_on_the_copy_it_places_and_writes_back_the_number_it_shows() {
        // The whole contract of a grip: where it sits is where the number it
        // drives puts it, so dragging it to a place and reading the parameter
        // back are the same operation.
        let mut params = with(&[
            ("kind", ParamValue::Choice(LINEAR)),
            ("count", ParamValue::Count(4)),
            ("step_x", ParamValue::Length(10.0)),
            ("step_y", ParamValue::Length(0.0)),
            ("step_z", ParamValue::Length(0.0)),
        ]);
        let spacing = grip(&params, "Spacing").expect("a run of four has a spacing to lay out");
        // At the last copy: three gaps of 10.
        assert!((spacing.at - Vec3::new(30.0, 0.0, 0.0)).length() < 1e-9, "{:?}", spacing.at);
        // Dragged out to 60, the three gaps become 20 each.
        apply_grip(&mut params, &spacing, 60.0);
        assert_eq!(params.get("step_x"), Some(&ParamValue::Length(20.0)));
        // And the grip has followed the copy it marks.
        assert!((grip(&params, "Spacing").unwrap().at - Vec3::new(60.0, 0.0, 0.0)).length() < 1e-9);

        // The copies grip sits one step past the last copy, and says how many
        // reach as far as it is dragged.
        let copies = grip(&params, "Copies").unwrap();
        assert!((copies.at - Vec3::new(80.0, 0.0, 0.0)).length() < 1e-9, "{:?}", copies.at);
        apply_grip(&mut params, &copies, 140.0);
        assert_eq!(params.get("count"), Some(&ParamValue::Count(7)), "seven 20 mm steps reach 140");
    }

    #[test]
    fn a_run_that_steps_diagonally_lengthens_along_its_own_diagonal() {
        let mut params = with(&[
            ("kind", ParamValue::Choice(LINEAR)),
            ("count", ParamValue::Count(3)),
            ("step_x", ParamValue::Length(3.0)),
            ("step_y", ParamValue::Length(4.0)),
            ("step_z", ParamValue::Length(0.0)),
        ]);
        let spacing = grip(&params, "Spacing").unwrap();
        // Two gaps of a 3-4-5 run: the grip is ten out along the diagonal.
        assert!((spacing.at - Vec3::new(6.0, 8.0, 0.0)).length() < 1e-9);
        apply_grip(&mut params, &spacing, 20.0);
        let step = Vec3::new(params.num("step_x"), params.num("step_y"), params.num("step_z"));
        assert!((step - Vec3::new(6.0, 8.0, 0.0)).length() < 1e-9, "straightened onto X: {step:?}");
    }

    #[test]
    fn a_count_grip_never_writes_a_number_the_property_editor_would_refuse() {
        let mut params = with(&[("kind", ParamValue::Choice(LINEAR)), ("step_x", ParamValue::Length(10.0))]);
        let copies = grip(&params, "Copies").unwrap();
        // Dragged back through the origin: one copy -- the original -- is the
        // floor, not zero and not a negative count.
        apply_grip(&mut params, &copies, -70.0);
        assert_eq!(params.get("count"), Some(&ParamValue::Count(1)));
        // And the far end stops at the same 512 the parameter itself allows.
        apply_grip(&mut params, &copies, 1_000_000.0);
        assert_eq!(params.get("count"), Some(&ParamValue::Count(512)));
    }

    #[test]
    fn every_kind_that_places_copies_offers_a_grip_for_each_number_that_places_them() {
        // Issue 67 asked for a way of laying a pattern out rather than only a
        // list of numbers. Each kind's grips are checked by name, and each one
        // is checked to sit where its own parameter says -- a grip in the wrong
        // place is a handle that jumps when it is grabbed.
        let grid = with(&[
            ("kind", ParamValue::Choice(GRID)),
            ("grid_x", ParamValue::Count(3)),
            ("grid_y", ParamValue::Count(2)),
            ("grid_z", ParamValue::Count(1)),
            ("grid_step_x", ParamValue::Length(10.0)),
            ("grid_step_y", ParamValue::Length(5.0)),
            ("grid_step_z", ParamValue::Length(4.0)),
        ]);
        assert_eq!(labels(&grid), vec!["Column spacing", "Columns", "Row spacing", "Rows", "Layers"]);
        assert!((grip(&grid, "Column spacing").unwrap().at - Vec3::new(20.0, 0.0, 0.0)).length() < 1e-9);
        assert!((grip(&grid, "Rows").unwrap().at - Vec3::new(0.0, 10.0, 0.0)).length() < 1e-9);
        // One layer has no spacing to drag -- there is no gap yet -- but the
        // count grip is there to pull a second one out of.
        assert!((grip(&grid, "Layers").unwrap().at - Vec3::new(0.0, 0.0, 4.0)).length() < 1e-9);

        let ring = with(&[
            ("kind", ParamValue::Choice(CIRCULAR)),
            ("circ_radius", ParamValue::Length(10.0)),
            ("circ_span", ParamValue::Angle(90.0)),
            ("circ_axis", ParamValue::Choice(2)),
        ]);
        assert_eq!(labels(&ring), vec!["Radius", "Span"]);
        assert!((grip(&ring, "Radius").unwrap().at - Vec3::new(10.0, 0.0, 0.0)).length() < 1e-9);
        // A quarter turn round, and out beyond the copies so a full turn does
        // not put it back on top of the radius grip.
        let span = grip(&ring, "Span").unwrap();
        assert!((span.at - Vec3::new(0.0, 13.0, 0.0)).length() < 1e-6, "{:?}", span.at);

        let helix = with(&[
            ("kind", ParamValue::Choice(HELIX)),
            ("helix_count", ParamValue::Count(5)),
            ("helix_radius", ParamValue::Length(10.0)),
            ("helix_rise", ParamValue::Length(4.0)),
            ("helix_axis", ParamValue::Choice(2)),
        ]);
        assert_eq!(labels(&helix), vec!["Radius", "Rise", "Copies", "Turn per copy"]);
        // The twist is taken hold of on the second copy, one turn from the
        // first, so where the grip is carried round to is the turn per copy.
        let turn = grip(&helix, "Turn per copy").unwrap();
        let Drive::Angle { per, .. } = turn.drive else { panic!("a turn grip must turn") };
        assert!((per - 1.0).abs() < 1e-9);
        assert!((turn.at.z - 4.0).abs() < 1e-9, "it left the second copy's level: {:?}", turn.at);
        // Both height grips ride the axis, not the turn: one on the top copy's
        // level, one a rise above it.
        assert!((grip(&helix, "Rise").unwrap().at - Vec3::new(0.0, 0.0, 16.0)).length() < 1e-9);
        assert!((grip(&helix, "Copies").unwrap().at - Vec3::new(0.0, 0.0, 20.0)).length() < 1e-9);

        let spiral = with(&[
            ("kind", ParamValue::Choice(SPIRAL)),
            ("spiral_count", ParamValue::Count(4)),
            ("spiral_radius", ParamValue::Length(5.0)),
            ("spiral_growth", ParamValue::Length(3.0)),
            ("spiral_rise", ParamValue::Length(0.0)),
            ("spiral_axis", ParamValue::Choice(2)),
        ]);
        // A ruler out from the centre: where it starts, where it ends, and one
        // copy beyond. No rise grip, because a flat spiral has no height to take
        // hold of.
        assert_eq!(labels(&spiral), vec!["Start radius", "Radius per copy", "Copies", "Turn per copy"]);
        assert!((grip(&spiral, "Radius per copy").unwrap().at - Vec3::new(14.0, 0.0, 0.0)).length() < 1e-9);
        assert!((grip(&spiral, "Copies").unwrap().at - Vec3::new(17.0, 0.0, 0.0)).length() < 1e-9);

        // A mirror is a plane and two copies: no distance, no count, no grips.
        assert!(labels(&with(&[("kind", ParamValue::Choice(MIRROR))])).is_empty());
    }

    #[test]
    fn a_grip_is_never_offered_on_the_patterns_own_origin() {
        // That is where the move manipulator sits, and two handles on one point
        // cannot both be grabbed. A run of no length puts every grip there.
        let flat = with(&[
            ("kind", ParamValue::Choice(LINEAR)),
            ("count", ParamValue::Count(4)),
            ("step_x", ParamValue::Length(0.0)),
            ("step_y", ParamValue::Length(0.0)),
            ("step_z", ParamValue::Length(0.0)),
        ]);
        assert!(labels(&flat).is_empty());
        // And a ring of no radius offers the radius to pull out, but no span
        // grip, which would sit on the centre too.
        let point = with(&[("kind", ParamValue::Choice(CIRCULAR)), ("circ_radius", ParamValue::Length(0.0))]);
        assert!(labels(&point).is_empty());
    }

    #[test]
    fn a_turning_grip_follows_the_axis_the_pattern_turns_about() {
        for axis in 0..3u32 {
            let ring = with(&[
                ("kind", ParamValue::Choice(CIRCULAR)),
                ("circ_radius", ParamValue::Length(10.0)),
                ("circ_span", ParamValue::Angle(360.0)),
                ("circ_axis", ParamValue::Choice(axis)),
            ]);
            let span = grip(&ring, "Span").expect("a ring with a radius has a span to drag");
            let Drive::Angle { axis: about, .. } = span.drive else { panic!("a span grip must turn") };
            assert_eq!(about, axis as usize);
            // It rides in the plane of the turn, so the axis component is zero.
            let along = match axis {
                0 => span.at.x,
                1 => span.at.y,
                _ => span.at.z,
            };
            assert!(along.abs() < 1e-9, "the span grip left the plane of its own ring: {:?}", span.at);
        }
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
