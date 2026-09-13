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
    migrate_noise(stored, &mut out);
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

/// Carry each stage's variations over from `stored`, however it wrote them.
///
/// A stage has three ways of having said what varies its copies, and each is
/// brought into the list the stage holds now:
///
/// * the list itself, whose parameters are named for their slot rather than
///   tabled, so they are copied across here -- checked against what each one
///   is, as the tabled ones are -- rather than by the table;
/// * four fixed slots, each a shift vector, a spin, a size and a gap on a cycle
///   that always left the original alone (see [`slot_variations`]);
/// * one of each kind under a single "Vary" heading (see [`legacy_variations`]).
fn migrate_stage_variations(stored: &Params, out: &mut Params) {
    for (index, k) in STAGES.iter().enumerate() {
        let list = if stored.contains_key(k.variations) {
            let used = stored.get(k.variations).map_or(0, |v| v.as_u32()).min(MAX_VARIATIONS) as usize;
            clear_variations_from(out, index, 0);
            for key in variation_keys(index, used) {
                // Only what is there, checked against what it is: a number a
                // variation's kind does not read is not written for it, and a
                // missing one reads as its default anyway.
                let Some(spec) = param_spec(&key) else { continue };
                let kept = stored
                    .get(&key)
                    .copied()
                    .filter(|v| std::mem::discriminant(v) == std::mem::discriminant(&spec.default));
                if let Some(value) = kept {
                    out.insert(key, value);
                }
            }
            out.insert(k.variations.to_string(), ParamValue::Count(used as u32));
            continue;
        } else if stored.contains_key(k.legacy_varied) {
            slot_variations(stored, index)
        } else {
            legacy_variations(stored, out, index)
        };
        let mode = StageMode::from_index(out.int(k.mode));
        let mut rebuilt = stage(out, index);
        rebuilt.vary = list.into_iter().filter(|variation| variation.what.fits(mode)).collect();
        set_stage(out, index, &rebuilt);
    }
}

/// What a stage's four fixed variation slots come to as a list (issue 79).
///
/// Each slot was a kind, a way of stepping, a cycle, and the numbers of all
/// four kinds -- a shift as a vector, a spin about an axis, a size in percent
/// and a gap. A shift along more than one axis is a variation along each, and a
/// cycle is what [`from_cycle`] makes of it, so every copy lands where it did.
fn slot_variations(stored: &Params, index: usize) -> Vec<Variation> {
    let k = &STAGES[index];
    let used = stored.get(k.legacy_varied).map_or(0, |v| v.as_u32()).min(4) as usize;
    let mut list = Vec::new();
    for slot in 1..=used {
        let key = |name: &str| format!("stage{}_vary{slot}_{name}", index + 1);
        let num = |name: &str| stored.get(&key(name)).map_or(0.0, |v| v.as_f64());
        let whole = |name: &str, default: u32| stored.get(&key(name)).map_or(default, |v| v.as_u32());
        let every = (whole("steps", 0) == 1).then(|| whole("every", 2));
        match Vary::from_index(whole("what", 0)) {
            Vary::Shift => {
                for (axis, name) in ["x", "y", "z"].into_iter().enumerate() {
                    if num(name).abs() > 1e-9 {
                        list.extend(from_cycle(Variation::shift(axis, num(name)), every));
                    }
                }
            }
            Vary::Spin => list.extend(from_cycle(Variation::spin(whole("axis", 2) as usize, num("angle")), every)),
            Vary::Size => {
                let factor = whole("size", 100).clamp(10, 1000) as f64 / 100.0;
                list.extend(from_cycle(Variation::resize(ALL_AXES, factor), every));
            }
            Vary::Gap => list.extend(from_cycle(Variation::widen(num("gap")), every)),
        }
    }
    list
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
/// heading. Each of those that was set becomes an entry of the list, stepping
/// the way it always did. They are listed in the order the old stage applied
/// them -- the gap places the copy, and it is then shifted, spun and resized
/// where it stands -- so a rule saved before lays its copies down where it
/// always did. A mirror never varied anything, and a turn never had gaps, so
/// neither is given what it did not use.
fn legacy_variations(stored: &Params, out: &Params, index: usize) -> Vec<Variation> {
    let k = &STAGES[index];
    let old = &k.legacy;
    let num = |key: &str| stored.get(key).map_or(0.0, |v| v.as_f64());
    let axis = out.int(k.axis).min(2) as usize;
    let mut list: Vec<Variation> = Vec::new();
    if num(old.gap_growth).abs() > 1e-9 {
        list.extend(from_cycle(Variation::widen(num(old.gap_growth)), None));
    }
    let every = stored.get(old.shift_every).map_or(2, |v| v.as_u32());
    for (shift_axis, key) in old.shift.iter().enumerate() {
        if num(key).abs() > 1e-9 {
            list.extend(from_cycle(Variation::shift(shift_axis, num(key)), Some(every)));
        }
    }
    if num(old.spin).abs() > 1e-9 {
        list.extend(from_cycle(Variation::spin(axis, num(old.spin)), None));
    }
    let scale = stored.get(old.scale).map_or(100, |v| v.as_u32()).clamp(10, 1000);
    if scale != 100 {
        list.extend(from_cycle(Variation::resize(ALL_AXES, scale as f64 / 100.0), None));
    }
    list
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
    // stage has and one its mode has a use for, then only the numbers its own
    // kind reads.
    if let Some((_, slot, field)) = parse_vary_key(key) {
        let what = Vary::from_index(values.int(&vary_key(stage, slot, VaryField::What)));
        if slot >= variation_count(values, stage) || !what.fits(mode) {
            return false;
        }
        return match field {
            VaryField::What | VaryField::Steps | VaryField::Every | VaryField::Start => true,
            VaryField::Axis => what != Vary::Gap,
            amount => amount == VaryField::amount_of(what),
        };
    }
    if key == k.variations {
        return mode != StageMode::Mirror;
    }
    match mode {
        StageMode::Move => key == k.count || k.step.contains(&key),
        StageMode::Turn => [k.count, k.axis, k.turn, k.radius, k.growth, k.rise].contains(&key),
        StageMode::Mirror => key == k.axis,
    }
}
