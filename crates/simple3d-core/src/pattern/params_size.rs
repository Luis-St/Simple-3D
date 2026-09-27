//! Fitting a new pattern to its shape, migrating older ones, and hiding unused parameters.

use super::*;
use crate::primitive::{ParamSpec, ParamValue, Params, ParamsExt};
use simple3d_geom::Vec3;

/// A fresh pattern's parameters, with distances scaled to the repeated shapes (issue 67). A fixed
/// 20 mm step equals the default box's width, laying copies face to face into a non-manifold lump.
pub fn params_for_size(size: Vec3) -> Params {
    let mut params = default_params();
    // 1.5x the extent, so each copy clears the previous one by half its width.
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
        // Custom stages also start from numbers suited to the shape.
        ("stage1_step_x", step(size.x)),
        ("stage2_step_y", step(size.y)),
        ("stage3_step_z", step(size.z)),
        ("stage4_radius", ParamValue::Length(radius * 2.0)),
    ] {
        params.insert(key.to_string(), value);
    }
    params
}

/// Fill in missing keys and drop unknown ones, migrating an older pattern like a primitive.
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

/// Update an older rule's stages: their modes and variations. Reads `stored` and writes `out`,
/// so it serves both project parameters and saved kinds with their different defaults.
pub fn migrate_stages(stored: &Params, out: &mut Params) {
    migrate_stage_modes(stored, out);
    migrate_stage_variations(stored, out);
}

/// Carry each stage's variations over from `stored`, whichever of three formats it used: the
/// current slot-named list (copied here, type-checked), four fixed slots ([`slot_variations`]),
/// or one of each kind under "Vary" ([`legacy_variations`]).
fn migrate_stage_variations(stored: &Params, out: &mut Params) {
    for (index, k) in STAGES.iter().enumerate() {
        let list = if stored.contains_key(k.variations) {
            let used = stored.get(k.variations).map_or(0, |v| v.as_u32()).min(MAX_VARIATIONS) as usize;
            clear_variations_from(out, index, 0);
            for key in variation_keys(index, used) {
                // Only present keys, type-checked; a missing one reads as its default anyway.
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

/// A stage's four old fixed variation slots as a list (issue 79). Multi-axis shifts become one
/// variation per axis, and cycles go through [`from_cycle`], so every copy lands where it did.
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

/// Derive each old stage's mode from its numbers (issue 79): a mirror flag is a mirror, a turn or
/// radius is a turn, anything else a run. A turning stage's rise was its step along its own axis.
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

/// An old stage's single "Vary" set as variations (issue 79), listed in the order the old stage
/// applied them (gap, shift, spin, size) so copies land where they did.
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

/// Whether a parameter is shown for the current values. Like primitives' choice gating, plus
/// custom stages gated on stage count and mode.
pub fn param_visible(spec: &ParamSpec, values: &Params) -> bool {
    let gated = match spec.shown_when {
        None => true,
        Some((key, want)) => values.int(key) == want,
    };
    gated && stage_param_visible(spec.key, values)
}

/// Whether a stage parameter applies: the stage must be in use and its mode must need it.
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
    // A variation's numbers (issue 79): only while the stage has it and its mode uses it, and only
    // those its kind reads.
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
