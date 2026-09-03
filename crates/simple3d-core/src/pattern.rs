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
pub const KINDS: &[&str] = &["Linear", "Grid", "Circular", "Mirror", "Helix", "Spiral", "Custom"];

const LINEAR: u32 = 0;
const GRID: u32 = 1;
const CIRCULAR: u32 = 2;
const MIRROR: u32 = 3;
const HELIX: u32 = 4;
const SPIRAL: u32 = 5;
/// A rule the user built themselves, out of stages (issue 67).
pub const CUSTOM: u32 = 6;

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

const fn flag(key: &'static str, label: &'static str, when: (&'static str, u32)) -> ParamSpec {
    ParamSpec {
        key,
        label,
        kind: ParamKind::Bool,
        default: ParamValue::Bool(false),
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
    // Custom: a rule the user builds themselves out of stages (issue 67). Each
    // stage repeats whatever the stages before it made, so one stage is a run,
    // two are a grid, and a run repeated round a turn is something none of the
    // fixed kinds above can say at all.
    count("stages", "Stages", 1, ("kind", CUSTOM)),
    flag("stage1_mirror", "1 Mirror", ("kind", CUSTOM)),
    axis("stage1_axis", ("kind", CUSTOM)),
    count("stage1_count", "1 Copies", 3, ("kind", CUSTOM)),
    length("stage1_step_x", "1 Step X", 20.0, ("kind", CUSTOM)),
    length("stage1_step_y", "1 Step Y", 0.0, ("kind", CUSTOM)),
    length("stage1_step_z", "1 Step Z", 0.0, ("kind", CUSTOM)),
    angle("stage1_turn", "1 Turn per copy", 0.0, ("kind", CUSTOM)),
    length("stage1_radius", "1 Radius", 0.0, ("kind", CUSTOM)),
    length("stage1_growth", "1 Radius per copy", 0.0, ("kind", CUSTOM)),
    flag("stage2_mirror", "2 Mirror", ("kind", CUSTOM)),
    axis("stage2_axis", ("kind", CUSTOM)),
    count("stage2_count", "2 Copies", 2, ("kind", CUSTOM)),
    length("stage2_step_x", "2 Step X", 0.0, ("kind", CUSTOM)),
    length("stage2_step_y", "2 Step Y", 20.0, ("kind", CUSTOM)),
    length("stage2_step_z", "2 Step Z", 0.0, ("kind", CUSTOM)),
    angle("stage2_turn", "2 Turn per copy", 0.0, ("kind", CUSTOM)),
    length("stage2_radius", "2 Radius", 0.0, ("kind", CUSTOM)),
    length("stage2_growth", "2 Radius per copy", 0.0, ("kind", CUSTOM)),
    flag("stage3_mirror", "3 Mirror", ("kind", CUSTOM)),
    axis("stage3_axis", ("kind", CUSTOM)),
    count("stage3_count", "3 Copies", 2, ("kind", CUSTOM)),
    length("stage3_step_x", "3 Step X", 0.0, ("kind", CUSTOM)),
    length("stage3_step_y", "3 Step Y", 0.0, ("kind", CUSTOM)),
    length("stage3_step_z", "3 Step Z", 20.0, ("kind", CUSTOM)),
    angle("stage3_turn", "3 Turn per copy", 0.0, ("kind", CUSTOM)),
    length("stage3_radius", "3 Radius", 0.0, ("kind", CUSTOM)),
    length("stage3_growth", "3 Radius per copy", 0.0, ("kind", CUSTOM)),
    flag("stage4_mirror", "4 Mirror", ("kind", CUSTOM)),
    axis("stage4_axis", ("kind", CUSTOM)),
    count("stage4_count", "4 Copies", 4, ("kind", CUSTOM)),
    length("stage4_step_x", "4 Step X", 0.0, ("kind", CUSTOM)),
    length("stage4_step_y", "4 Step Y", 0.0, ("kind", CUSTOM)),
    length("stage4_step_z", "4 Step Z", 0.0, ("kind", CUSTOM)),
    angle("stage4_turn", "4 Turn per copy", 90.0, ("kind", CUSTOM)),
    length("stage4_radius", "4 Radius", 40.0, ("kind", CUSTOM)),
    length("stage4_growth", "4 Radius per copy", 0.0, ("kind", CUSTOM)),
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

/// The most stages one custom kind is built from.
///
/// Four is what a rule anyone builds by hand actually needs -- three axes and a
/// turn, which is already more than any of the fixed kinds says -- and a fixed
/// number is what lets a stage's numbers be ordinary parameters with names of
/// their own. That is not a detail: it is what makes the property editor render
/// them, the project file carry them, the clipboard copy them and undo cover
/// them, none of which needed a line of code here.
pub const MAX_STAGES: usize = 4;

/// The parameter names one stage owns, and the names its viewport handles go by.
///
/// A table rather than names built with `format!` at each use: the parameter
/// keys and the grip labels have to be `'static` to be a [`ParamSpec`] and a
/// [`Grip`], and having them in one place is what keeps the maths, the editor
/// and the handles talking about the same stage.
pub struct StageKeys {
    pub label: &'static str,
    pub mirror: &'static str,
    pub axis: &'static str,
    pub count: &'static str,
    pub step: [&'static str; 3],
    pub turn: &'static str,
    pub radius: &'static str,
    pub growth: &'static str,
    grip_spacing: &'static str,
    grip_copies: &'static str,
    grip_radius: &'static str,
}

pub const STAGES: [StageKeys; MAX_STAGES] = [
    StageKeys {
        label: "Stage 1",
        mirror: "stage1_mirror",
        axis: "stage1_axis",
        count: "stage1_count",
        step: ["stage1_step_x", "stage1_step_y", "stage1_step_z"],
        turn: "stage1_turn",
        radius: "stage1_radius",
        growth: "stage1_growth",
        grip_spacing: "Stage 1 spacing",
        grip_copies: "Stage 1 copies",
        grip_radius: "Stage 1 radius",
    },
    StageKeys {
        label: "Stage 2",
        mirror: "stage2_mirror",
        axis: "stage2_axis",
        count: "stage2_count",
        step: ["stage2_step_x", "stage2_step_y", "stage2_step_z"],
        turn: "stage2_turn",
        radius: "stage2_radius",
        growth: "stage2_growth",
        grip_spacing: "Stage 2 spacing",
        grip_copies: "Stage 2 copies",
        grip_radius: "Stage 2 radius",
    },
    StageKeys {
        label: "Stage 3",
        mirror: "stage3_mirror",
        axis: "stage3_axis",
        count: "stage3_count",
        step: ["stage3_step_x", "stage3_step_y", "stage3_step_z"],
        turn: "stage3_turn",
        radius: "stage3_radius",
        growth: "stage3_growth",
        grip_spacing: "Stage 3 spacing",
        grip_copies: "Stage 3 copies",
        grip_radius: "Stage 3 radius",
    },
    StageKeys {
        label: "Stage 4",
        mirror: "stage4_mirror",
        axis: "stage4_axis",
        count: "stage4_count",
        step: ["stage4_step_x", "stage4_step_y", "stage4_step_z"],
        turn: "stage4_turn",
        radius: "stage4_radius",
        growth: "stage4_growth",
        grip_spacing: "Stage 4 spacing",
        grip_copies: "Stage 4 copies",
        grip_radius: "Stage 4 radius",
    },
];

/// Every parameter key a stage owns, in the order the editor shows them.
pub fn stage_keys(stage: usize) -> Vec<&'static str> {
    let k = &STAGES[stage.min(MAX_STAGES - 1)];
    vec![k.mirror, k.axis, k.count, k.step[0], k.step[1], k.step[2], k.turn, k.radius, k.growth]
}

/// How many stages a custom rule is currently using.
pub fn stage_count(params: &Params) -> usize {
    params.int("stages").clamp(1, MAX_STAGES as u32) as usize
}

/// The stage a parameter key belongs to, zero-based -- `None` for every key
/// that is not one of a stage's own.
fn stage_of(key: &str) -> Option<usize> {
    let digit = key.strip_prefix("stage")?.as_bytes().first().copied()?;
    let index = digit.checked_sub(b'1')? as usize;
    (index < MAX_STAGES).then_some(index)
}

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
        // The custom stages get the same treatment: a rule built by hand starts
        // from numbers that suit what it is repeating, not from a stock 20 mm.
        ("stage1_step_x", step(size.x)),
        ("stage2_step_y", step(size.y)),
        ("stage3_step_z", step(size.z)),
        ("stage4_radius", ParamValue::Length(radius * 2.0)),
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
/// same rule a primitive's choice-gated parameters follow, plus the one thing a
/// primitive never needs: a custom rule's stages are gated on *how many* stages
/// there are, which is a comparison rather than the equality `shown_when` says.
pub fn param_visible(spec: &ParamSpec, values: &Params) -> bool {
    let gated = match spec.shown_when {
        None => true,
        Some((key, want)) => values.int(key) == want,
    };
    gated && stage_param_visible(spec.key, values)
}

/// Whether a stage's parameter applies: the stage has to be one of the ones in
/// use, and a stage set to mirror is a plane and two copies -- none of the
/// numbers that place a run mean anything for it.
fn stage_param_visible(key: &str, values: &Params) -> bool {
    let Some(stage) = stage_of(key) else { return true };
    if stage >= stage_count(values) {
        return false;
    }
    let k = &STAGES[stage];
    if key == k.mirror || key == k.axis {
        return true;
    }
    !values.flag(k.mirror)
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
        CUSTOM => custom(params),
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
        // Every stage repeats what the ones before it made, so the copies
        // multiply exactly as a grid's three counts do.
        CUSTOM => (0..stage_count(params))
            .map(|s| stage(params, s).copies())
            .fold(1usize, |total, copies| total.saturating_mul(copies)),
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

// -- custom kinds (issue 67) -------------------------------------------------
//
// The six kinds above are the ones worth having a name for. A custom kind is
// the rule underneath all of them, spelled out: a stack of *stages*, each one
// repeating whatever the stages before it made. One stage stepping along X is a
// linear pattern; a second stepping along Y makes it a grid; a stage that turns
// about Z at a radius makes a ring, and a ring of rows is something no fixed
// kind can say. Every kind above can be written as one or two stages, which is
// the check that the model is the right one rather than a seventh special case.

/// One stage of a custom rule: how many copies it makes, and what it does to
/// each of them.
///
/// The transform of copy `i` is worked out from `i` directly rather than by
/// composing the stage with itself `i` times. That is what makes a stage able to
/// say "the radius grows 5 mm a copy" -- repeated composition would carry the
/// growth round the turn with it and draw an involute instead of a spiral -- and
/// it is the same arithmetic the fixed kinds do, so they come out identical.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage {
    pub count: u32,
    /// Moved this far further along for each copy.
    pub step: Vec3,
    /// Turned this much further about `axis` for each copy.
    pub turn: f64,
    pub axis: usize,
    /// How far out from the axis the first copy sits.
    pub radius: f64,
    /// How much further out each copy after it sits.
    pub growth: f64,
    /// The stage is a reflection across the plane through the origin whose
    /// normal is `axis`: the original and its mirror image, and nothing else.
    pub mirror: bool,
}

impl Stage {
    /// The copies this stage makes, in the frame of whatever it is repeating.
    pub fn instances(&self) -> Vec<Instance> {
        if self.mirror {
            let mut m = Xform::IDENTITY;
            m.m[self.axis][self.axis] = -1.0;
            return vec![Instance::plain(Xform::IDENTITY), Instance { xform: m, mirrored: true }];
        }
        (0..self.count.max(1))
            .map(|i| {
                let i = i as f64;
                let placed = turned(self.axis, self.radius + self.growth * i, self.turn * i, 0.0);
                Instance::plain(Xform::from_translation(self.step * i).compose(&placed))
            })
            .collect()
    }

    /// How many copies it makes. A mirror is always two.
    pub fn copies(&self) -> usize {
        if self.mirror {
            2
        } else {
            self.count.max(1) as usize
        }
    }

    /// Whether the stage does anything at all: one copy that does not move is a
    /// stage the user has not filled in yet.
    pub fn is_idle(&self) -> bool {
        !self.mirror && self.count.max(1) == 1
    }
}

/// Read one stage out of a pattern's parameters.
pub fn stage(params: &Params, index: usize) -> Stage {
    let k = &STAGES[index.min(MAX_STAGES - 1)];
    Stage {
        count: params.int(k.count).max(1),
        step: Vec3::new(params.num(k.step[0]), params.num(k.step[1]), params.num(k.step[2])),
        turn: params.num(k.turn),
        axis: params.int(k.axis).min(2) as usize,
        radius: params.num(k.radius),
        growth: params.num(k.growth),
        mirror: params.flag(k.mirror),
    }
}

/// Write one stage back into a pattern's parameters.
pub fn set_stage(params: &mut Params, index: usize, stage: Stage) {
    let k = &STAGES[index.min(MAX_STAGES - 1)];
    params.insert(k.count.to_string(), ParamValue::Count(stage.count.clamp(1, 512)));
    for (axis, key) in k.step.iter().enumerate() {
        let component = match axis {
            0 => stage.step.x,
            1 => stage.step.y,
            _ => stage.step.z,
        };
        params.insert((*key).to_string(), ParamValue::Length(component));
    }
    params.insert(k.turn.to_string(), ParamValue::Angle(stage.turn.clamp(-360.0, 360.0)));
    params.insert(k.axis.to_string(), ParamValue::Choice(stage.axis.min(2) as u32));
    params.insert(k.radius.to_string(), ParamValue::Length(stage.radius));
    params.insert(k.growth.to_string(), ParamValue::Length(stage.growth));
    params.insert(k.mirror.to_string(), ParamValue::Bool(stage.mirror));
}

/// Every parameter a custom rule is made of, which is what a saved kind holds
/// and what applying one writes.
pub fn custom_keys() -> Vec<&'static str> {
    let mut keys = vec!["stages"];
    for index in 0..MAX_STAGES {
        keys.extend(stage_keys(index));
    }
    keys
}

fn custom(params: &Params) -> Vec<Instance> {
    // Start with the shape itself, and let each stage repeat everything that
    // came before it. The outer transform is the later stage's, so "a row of
    // five, turned four times round Z" turns the whole row rather than each
    // copy where it stands.
    let mut out = vec![Instance::plain(Xform::IDENTITY)];
    for index in 0..stage_count(params) {
        let stage = stage(params, index);
        let mut next: Vec<Instance> = Vec::new();
        'fill: for outer in stage.instances() {
            for inner in &out {
                // The cap is checked while building rather than by truncating
                // afterwards, for the reason a grid checks it: four stages of
                // 512 multiply to more transforms than there is memory for.
                if next.len() >= MAX_INSTANCES {
                    break 'fill;
                }
                next.push(Instance {
                    xform: outer.xform.compose(&inner.xform),
                    // A reflection of a reflection points outward again.
                    mirrored: outer.mirrored != inner.mirrored,
                });
            }
        }
        out = next;
    }
    out
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
        CUSTOM => custom_grips(params),
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

/// A custom rule's handles: the run and the radius of every stage in use.
///
/// One stage's numbers are laid out exactly as a linear pattern's are, because
/// that is what a stage stepping along a line is. The turn is left as a number:
/// a stage may step *and* turn at once, and a handle riding a curve that its own
/// neighbour is also moving is one nobody can aim at.
fn custom_grips(params: &Params) -> Vec<Grip> {
    let mut out = Vec::new();
    for (index, k) in STAGES.iter().enumerate().take(stage_count(params)) {
        let stage = stage(params, index);
        if stage.mirror {
            // A mirror is a plane and two copies: no distance, nothing to drag.
            continue;
        }
        let length = stage.step.length();
        let dir = if length > 1e-9 { stage.step * (1.0 / length) } else { unit(0) };
        if length > 1e-9 && stage.count >= 2 {
            out.push(Grip::slide(
                k.grip_spacing,
                stage.step * (stage.count - 1) as f64,
                dir,
                Drive::Length { keys: &k.step, base: 0.0, per: (stage.count - 1) as f64, min: POSITIVE },
            ));
        }
        if length > 1e-9 {
            out.push(Grip::slide(
                k.grip_copies,
                dir * (length * stage.count as f64),
                dir,
                Drive::Count { key: k.count, base: 0.0, per: length },
            ));
        }
        if stage.radius.abs() > 1e-9 {
            let radial = unit(radial_axis(stage.axis));
            out.push(Grip::slide(
                k.grip_radius,
                radial * stage.radius,
                radial,
                Drive::Length { keys: std::slice::from_ref(&k.radius), base: 0.0, per: 1.0, min: POSITIVE },
            ));
        }
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

    /// A custom rule with one stage set to step along X is a linear pattern.
    /// Not "close to one" -- the same transforms, copy for copy, which is what
    /// says the stage model is the rule the fixed kinds are special cases of
    /// rather than a seventh thing that happens to look similar (issue 67).
    #[test]
    fn one_custom_stage_says_exactly_what_a_linear_pattern_says() {
        let linear = with(&[
            ("kind", ParamValue::Choice(LINEAR)),
            ("count", ParamValue::Count(4)),
            ("step_x", ParamValue::Length(10.0)),
        ]);
        let custom = with(&[
            ("kind", ParamValue::Choice(CUSTOM)),
            ("stages", ParamValue::Count(1)),
            ("stage1_count", ParamValue::Count(4)),
            ("stage1_step_x", ParamValue::Length(10.0)),
        ]);
        assert_eq!(instances(&custom), instances(&linear));
    }

    /// Two stages stepping along two axes are a grid, in the same order.
    #[test]
    fn two_custom_stages_say_exactly_what_a_grid_says() {
        let grid = with(&[
            ("kind", ParamValue::Choice(GRID)),
            ("grid_x", ParamValue::Count(3)),
            ("grid_y", ParamValue::Count(2)),
            ("grid_z", ParamValue::Count(1)),
            ("grid_step_x", ParamValue::Length(10.0)),
            ("grid_step_y", ParamValue::Length(5.0)),
        ]);
        let custom = with(&[
            ("kind", ParamValue::Choice(CUSTOM)),
            ("stages", ParamValue::Count(2)),
            ("stage1_count", ParamValue::Count(3)),
            ("stage1_step_x", ParamValue::Length(10.0)),
            ("stage2_count", ParamValue::Count(2)),
            ("stage2_step_y", ParamValue::Length(5.0)),
        ]);
        assert_eq!(instances(&custom), instances(&grid));
    }

    /// A stage that turns at a radius is a ring, and a stage that turns while
    /// rising is a helix.
    #[test]
    fn a_turning_stage_says_what_a_ring_and_a_helix_say() {
        let ring = with(&[
            ("kind", ParamValue::Choice(CIRCULAR)),
            ("circ_count", ParamValue::Count(6)),
            ("circ_span", ParamValue::Angle(360.0)),
            ("circ_radius", ParamValue::Length(25.0)),
        ]);
        let as_stage = with(&[
            ("kind", ParamValue::Choice(CUSTOM)),
            ("stage1_count", ParamValue::Count(6)),
            ("stage1_turn", ParamValue::Angle(60.0)),
            ("stage1_radius", ParamValue::Length(25.0)),
            ("stage1_step_x", ParamValue::Length(0.0)),
        ]);
        assert_eq!(instances(&as_stage), instances(&ring));

        let helix = with(&[
            ("kind", ParamValue::Choice(HELIX)),
            ("helix_count", ParamValue::Count(5)),
            ("helix_angle", ParamValue::Angle(45.0)),
            ("helix_rise", ParamValue::Length(4.0)),
            ("helix_radius", ParamValue::Length(20.0)),
        ]);
        let as_stage = with(&[
            ("kind", ParamValue::Choice(CUSTOM)),
            ("stage1_count", ParamValue::Count(5)),
            ("stage1_turn", ParamValue::Angle(45.0)),
            ("stage1_radius", ParamValue::Length(20.0)),
            ("stage1_step_x", ParamValue::Length(0.0)),
            ("stage1_step_z", ParamValue::Length(4.0)),
        ]);
        assert_eq!(instances(&as_stage), instances(&helix));
    }

    /// A stage that grows its radius while it turns is a spiral.
    #[test]
    fn a_growing_stage_says_what_a_spiral_says() {
        let spiral = with(&[
            ("kind", ParamValue::Choice(SPIRAL)),
            ("spiral_count", ParamValue::Count(7)),
            ("spiral_angle", ParamValue::Angle(30.0)),
            ("spiral_radius", ParamValue::Length(10.0)),
            ("spiral_growth", ParamValue::Length(5.0)),
            ("spiral_rise", ParamValue::Length(0.0)),
        ]);
        let as_stage = with(&[
            ("kind", ParamValue::Choice(CUSTOM)),
            ("stage1_count", ParamValue::Count(7)),
            ("stage1_turn", ParamValue::Angle(30.0)),
            ("stage1_radius", ParamValue::Length(10.0)),
            ("stage1_growth", ParamValue::Length(5.0)),
            ("stage1_step_x", ParamValue::Length(0.0)),
        ]);
        assert_eq!(instances(&as_stage), instances(&spiral));
    }

    /// A mirror stage is a mirror pattern, and mirroring twice points the faces
    /// back outward rather than leaving them inside out.
    #[test]
    fn a_mirror_stage_reflects_and_two_of_them_cancel() {
        let mirrored = with(&[("kind", ParamValue::Choice(MIRROR)), ("mirror_axis", ParamValue::Choice(0))]);
        let as_stage = with(&[
            ("kind", ParamValue::Choice(CUSTOM)),
            ("stage1_mirror", ParamValue::Bool(true)),
            ("stage1_axis", ParamValue::Choice(0)),
        ]);
        assert_eq!(instances(&as_stage), instances(&mirrored));

        let twice = with(&[
            ("kind", ParamValue::Choice(CUSTOM)),
            ("stages", ParamValue::Count(2)),
            ("stage1_mirror", ParamValue::Bool(true)),
            ("stage1_axis", ParamValue::Choice(0)),
            ("stage2_mirror", ParamValue::Bool(true)),
            ("stage2_axis", ParamValue::Choice(1)),
        ]);
        let copies = instances(&twice);
        assert_eq!(copies.len(), 4);
        assert_eq!(copies.iter().filter(|c| c.mirrored).count(), 2, "a reflection of a reflection points out again");
    }

    /// The point of stages: a later one repeats what the earlier ones *made*.
    /// A row of three, turned four times about Z, is four rows standing round a
    /// centre -- not four copies of the first shape and three of nothing.
    #[test]
    fn a_later_stage_repeats_what_the_earlier_ones_made() {
        let params = with(&[
            ("kind", ParamValue::Choice(CUSTOM)),
            ("stages", ParamValue::Count(2)),
            ("stage1_count", ParamValue::Count(3)),
            ("stage1_step_x", ParamValue::Length(10.0)),
            ("stage2_count", ParamValue::Count(4)),
            ("stage2_turn", ParamValue::Angle(90.0)),
            ("stage2_step_x", ParamValue::Length(0.0)),
        ]);
        let copies = instances(&params);
        assert_eq!(copies.len(), 12);
        assert_eq!(instance_count(&params), (12, 12));
        // The row runs out along X; a quarter turn about Z carries it onto Y,
        // so the far end of the second row is 20 mm up the Y axis.
        assert!(
            copies.iter().any(|c| (c.xform.t - Vec3::new(0.0, 20.0, 0.0)).length() < 1e-9),
            "the second stage turned the copies rather than the row"
        );
    }

    /// Four stages of 512 multiply to more transforms than there is memory for,
    /// so the cap has to bite while the copies are being built.
    #[test]
    fn a_custom_rule_cannot_ask_for_more_copies_than_anything_can_draw() {
        let mut params = with(&[("kind", ParamValue::Choice(CUSTOM)), ("stages", ParamValue::Count(4))]);
        for keys in &STAGES {
            params.insert(keys.count.to_string(), ParamValue::Count(512));
            params.insert(keys.step[0].to_string(), ParamValue::Length(1.0));
        }
        let (wanted, made) = instance_count(&params);
        assert_eq!(wanted, 512usize.pow(4));
        assert_eq!(made, MAX_INSTANCES);
        assert_eq!(instances(&params).len(), MAX_INSTANCES);
    }

    /// Only the stages in use are shown, a mirror stage shows nothing but the
    /// plane it reflects across, and no stage at all is shown for another kind.
    #[test]
    fn a_stages_fields_are_shown_only_while_that_stage_is_in_use() {
        let shown = |params: &Params| -> Vec<&'static str> {
            PARAMS.iter().filter(|p| param_visible(p, params)).map(|p| p.key).collect()
        };
        let linear = with(&[("kind", ParamValue::Choice(LINEAR))]);
        assert!(shown(&linear).iter().all(|k| !k.starts_with("stage")));

        let one = with(&[("kind", ParamValue::Choice(CUSTOM)), ("stages", ParamValue::Count(1))]);
        assert!(shown(&one).contains(&"stage1_count"));
        assert!(!shown(&one).contains(&"stage2_count"), "a stage that is not in use was still shown");

        let mirrored = with(&[("kind", ParamValue::Choice(CUSTOM)), ("stage1_mirror", ParamValue::Bool(true))]);
        assert!(shown(&mirrored).contains(&"stage1_axis"), "a mirror stage still chooses its plane");
        assert!(!shown(&mirrored).contains(&"stage1_step_x"), "a mirror stage has no run to place");
    }

    /// Every key the stage table names has to be a parameter that exists, or a
    /// handle would drive a number nothing shows and nothing saves.
    #[test]
    fn every_stage_key_is_a_parameter_of_its_own() {
        let known: Vec<&'static str> = PARAMS.iter().map(|p| p.key).collect();
        for key in custom_keys() {
            assert!(known.contains(&key), "{key} is named by the stage table but is not a parameter");
        }
        assert_eq!(custom_keys().len(), 1 + MAX_STAGES * 9);
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
