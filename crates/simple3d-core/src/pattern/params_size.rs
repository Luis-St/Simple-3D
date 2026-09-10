//! Fitting a new pattern to the shape it was made from, migrating an older
//! one, and hiding the parameters a kind does not use.

use super::*;
use crate::primitive::{ParamSpec, ParamValue, Params, ParamsExt};
use simple3d_geom::Vec3;

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
    let mut out: Params = PARAMS
        .iter()
        .map(|p| {
            let value = stored
                .get(p.key)
                .copied()
                .filter(|v| std::mem::discriminant(v) == std::mem::discriminant(&p.default))
                .unwrap_or(p.default);
            (p.key.to_string(), value)
        })
        .collect();
    migrate_stage_modes(stored, &mut out);
    out
}

/// Work out what each stage of an older rule was doing, and say so (issue 79).
///
/// A stage used to be nine numbers with a mirror flag on the front and no word
/// for what it was for; it is now a choice of three and the numbers that choice
/// needs. A rule saved before the choice existed still has to lay its copies
/// down in the same places, so the choice is *derived* from what the old numbers
/// said: a flag set is a mirror, a turn or a radius is a turn, and anything else
/// is a run. A turning stage's rise used to be its step along its own axis,
/// which is where the rise comes from.
///
/// Reads `stored` and writes `out` -- so it works both for a project's
/// parameters and for a saved kind on the shelf, which are filled in from
/// different defaults.
pub fn migrate_stage_modes(stored: &Params, out: &mut Params) {
    for k in &STAGES {
        if stored.contains_key(k.mode) {
            continue;
        }
        let axis = stored.get(k.axis).map_or(2, |v| v.as_u32()).min(2) as usize;
        let numbers = [k.turn, k.radius, k.growth].map(|key| stored.get(key).map_or(0.0, |v| v.as_f64()));
        let mode = if stored.get(k.legacy_mirror).is_some_and(|v| v.as_bool()) {
            StageMode::Mirror
        } else if numbers.iter().any(|n| n.abs() > 1e-9) {
            StageMode::Turn
        } else {
            StageMode::Move
        };
        out.insert(k.mode.to_string(), ParamValue::Choice(mode.index()));
        if mode == StageMode::Turn {
            let rise = stored.get(k.step[axis]).map_or(0.0, |v| v.as_f64());
            out.insert(k.rise.to_string(), ParamValue::Length(rise));
        }
    }
}

/// Whether a parameter should be shown, given the kind currently chosen. The
/// same rule a primitive's choice-gated parameters follow, plus the one thing a
/// primitive never needs: a custom rule's stages are gated on *how many* stages
/// there are and on what each one does, which is more than the equality
/// `shown_when` says.
pub fn param_visible(spec: &ParamSpec, values: &Params) -> bool {
    let gated = match spec.shown_when {
        None => true,
        Some((key, want)) => values.int(key) == want,
    };
    gated && stage_param_visible(spec.key, values)
}

/// Whether a stage's parameter applies: the stage has to be one of the ones in
/// use, and then it has to be one of the numbers the stage's own mode needs --
/// a run has no radius, and a mirror is a plane and two copies.
pub(crate) fn stage_param_visible(key: &str, values: &Params) -> bool {
    let Some(stage) = stage_of(key) else { return true };
    if stage >= stage_count(values) {
        return false;
    }
    let k = &STAGES[stage];
    if key == k.mode {
        return true;
    }
    match StageMode::from_index(values.int(k.mode)) {
        StageMode::Move => key == k.count || k.step.contains(&key),
        StageMode::Turn => [k.count, k.axis, k.turn, k.radius, k.growth, k.rise].contains(&key),
        StageMode::Mirror => key == k.axis,
    }
}
