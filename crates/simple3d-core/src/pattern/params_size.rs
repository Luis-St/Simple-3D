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
    migrate_stages(stored, &mut out);
    out
}

/// Bring an older rule's stages up to date: what each one does, and what it
/// varies its copies by.
///
/// Reads `stored` and writes `out` -- so it works both for a project's
/// parameters and for a saved kind on the shelf, which are filled in from
/// different defaults.
pub fn migrate_stages(stored: &Params, out: &mut Params) {
    migrate_stage_modes(stored, out);
    migrate_stage_variations(stored, out);
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
fn migrate_stage_modes(stored: &Params, out: &mut Params) {
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

/// Turn what an older stage varied its copies by into variations of its own
/// (issue 79).
///
/// A stage used to carry one of each -- a gap that grew, a shift on a cycle, a
/// spin and a size that built up copy by copy -- under a single "Vary"
/// heading. It now holds a list, and each of those that was set becomes one
/// entry of it, stepping the way it always did. They are listed in the order
/// the old stage applied them -- the gap places the copy, and it is then
/// shifted, spun and resized where it stands -- so a rule saved before lays
/// its copies down where it always did. A mirror never varied anything, and a
/// turn never had gaps, so neither is given what it did not use.
fn migrate_stage_variations(stored: &Params, out: &mut Params) {
    for k in &STAGES {
        if stored.contains_key(k.varied) {
            continue;
        }
        let old = &k.legacy;
        let num = |key: &str| stored.get(key).map_or(0.0, |v| v.as_f64());
        let mode = StageMode::from_index(out.int(k.mode));
        let axis = stored.get(k.axis).map_or(2, |v| v.as_u32()).min(2) as usize;
        let mut list: Vec<Variation> = Vec::new();
        if num(old.gap_growth).abs() > 1e-9 {
            list.push(Variation::widen(num(old.gap_growth)));
        }
        let shift = Vec3::new(num(old.shift[0]), num(old.shift[1]), num(old.shift[2]));
        if shift.length() > 1e-9 {
            list.push(Variation::shift(shift).repeating(stored.get(old.shift_every).map_or(2, |v| v.as_u32())));
        }
        if num(old.spin).abs() > 1e-9 {
            list.push(Variation::spin(num(old.spin), axis));
        }
        let scale = stored.get(old.scale).map_or(100, |v| v.as_u32()).clamp(10, 1000);
        if scale != 100 {
            list.push(Variation::resize(scale as f64 / 100.0));
        }
        list.retain(|variation| variation.what.fits(mode));
        out.insert(k.varied.to_string(), ParamValue::Count(list.len() as u32));
        for (slot, keys) in k.vary.iter().enumerate() {
            let blank = Variation::blank(Vary::Shift);
            write_variation(out, keys, list.get(slot).unwrap_or(&blank));
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
/// a run has no radius, and a mirror is a plane and two copies that nothing
/// varies.
pub(crate) fn stage_param_visible(key: &str, values: &Params) -> bool {
    let Some(stage) = stage_of(key) else { return true };
    if stage >= stage_count(values) {
        return false;
    }
    let k = &STAGES[stage];
    if key == k.mode {
        return true;
    }
    let mode = StageMode::from_index(values.int(k.mode));
    // A variation's numbers (issue 79): only while the variation is one the
    // stage uses and one its mode has a use for, then only the numbers its own
    // kind reads -- and its cycle only once it repeats.
    if let Some(slot) = k.vary.iter().position(|v| v.all().contains(&key)) {
        let v = &k.vary[slot];
        let what = Vary::from_index(values.int(v.what));
        if slot >= variation_count(values, stage) || !what.fits(mode) {
            return false;
        }
        return match what {
            _ if key == v.what || key == v.steps => true,
            _ if key == v.every => values.int(v.steps) == 1,
            Vary::Shift => v.offset.contains(&key),
            Vary::Spin => key == v.angle || key == v.axis,
            Vary::Size => key == v.size,
            Vary::Gap => key == v.gap,
        };
    }
    if key == k.varied {
        return mode != StageMode::Mirror;
    }
    match mode {
        StageMode::Move => key == k.count || k.step.contains(&key),
        StageMode::Turn => [k.count, k.axis, k.turn, k.radius, k.growth, k.rise].contains(&key),
        StageMode::Mirror => key == k.axis,
    }
}
