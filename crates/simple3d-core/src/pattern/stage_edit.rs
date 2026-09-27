//! Adding, dropping and reordering the stages of a custom rule (issue 79).

use super::*;
use crate::primitive::{ParamValue, Params};
use simple3d_geom::Vec3;

/// The numbers a newly added stage starts with: the next thing the rule lacks, a run along the
/// first free axis clear of the shape, then a ring. Previously it kept its slot's old contents.
pub fn fresh_stage(params: &Params, index: usize, size: Vec3) -> Stage {
    let mut taken = [false; 3];
    for above in (0..index.min(MAX_STAGES)).map(|i| stage(params, i)) {
        if above.mode == StageMode::Move && above.step.length() > 1e-9 {
            taken[along_axis(above.step)] = true;
        }
    }
    let extents = [size.x, size.y, size.z];
    match (0..3).find(|axis| !taken[*axis]) {
        Some(axis) => Stage::run(2, unit(axis) * clear(extents[axis])),
        None => fresh_turn(size),
    }
}

/// A new stage's numbers when its mode was chosen with it (issue 79): a run as [`fresh_stage`], a
/// ring round the shape, or a mirror across X beside the copies above.
pub fn fresh_stage_doing(params: &Params, index: usize, size: Vec3, mode: StageMode) -> Stage {
    match mode {
        StageMode::Move => match fresh_stage(params, index, size) {
            run if run.mode == StageMode::Move => run,
            _ => Stage::run(2, unit(0) * clear(size.x)),
        },
        StageMode::Turn => fresh_turn(size),
        StageMode::Mirror => Stage::mirrored(0),
    }
}

/// A newly added variation of `what` (issue 79), visible and sized to the shape and stage: a shift
/// staggers by half the nearest run's step (or zigzags), a spin builds 6 degrees a copy, a size
/// shrinks a tenth, a gap widens a quarter step. If the stage has one just like it, the nearest
/// different one; `None` if none is left.
pub fn fresh_variation(params: &Params, index: usize, what: Vary, size: Vec3) -> Option<Variation> {
    let own = stage(params, index);
    let extents = [size.x, size.y, size.z];
    let half_shape = |axis: usize| if extents[axis] > 1e-9 { extents[axis] / 2.0 } else { 10.0 };
    let wanted = match what {
        Vary::Shift => {
            let above = (0..index)
                .rev()
                .map(|i| stage(params, i))
                .find(|s| s.mode == StageMode::Move && s.step.length() > 1e-9);
            let (axis, distance) = match above {
                Some(run) => {
                    let axis = along_axis(run.step);
                    (axis, [run.step.x, run.step.y, run.step.z][axis] * 0.5)
                }
                None => {
                    let across = match own.mode {
                        StageMode::Turn => radial_axis(own.axis),
                        _ if own.step.length() > 1e-9 && along_axis(own.step) == 0 => 1,
                        _ => 0,
                    };
                    (across, half_shape(across))
                }
            };
            Variation::shift(axis, distance).repeating(2)
        }
        Vary::Spin => Variation::spin(if own.mode == StageMode::Turn { own.axis } else { 2 }, 15.0),
        Vary::Size => Variation::resize(ALL_AXES, 0.9),
        Vary::Gap => {
            let length = own.step.length();
            Variation::widen(if length > 1e-9 { length * 0.25 } else { clear(size.x) * 0.25 })
        }
    };
    if !has_room_for(&own, what) {
        return None;
    }
    free_variation(&own, wanted)
}

/// Append `variation` to stage `index`; false and unchanged if the stage has one just like it
/// ([`Variation::combination`]).
pub fn add_variation(params: &mut Params, index: usize, variation: Variation) -> bool {
    let stage = stage(params, index);
    if stage.variations().iter().any(|held| held.combination() == variation.combination()) {
        return false;
    }
    set_stage(params, index, &stage.with(variation));
    true
}

/// Remove variation `slot` from stage `index`; the rest keep their order.
pub fn remove_variation(params: &mut Params, index: usize, slot: usize) {
    let mut stage = stage(params, index);
    if slot >= stage.vary.len() {
        return;
    }
    stage.vary.remove(slot);
    set_stage(params, index, &stage);
}

/// A ring round the shape: four copies a right angle apart, clear of each other.
fn fresh_turn(size: Vec3) -> Stage {
    Stage::turning(4, 90.0, clear(size.x.max(size.y)) * 2.0, 0.0, 0.0, 2)
}

/// 1.5x the extent, the spacing `params_for_size` uses: a visible gap at any size.
fn clear(extent: f64) -> f64 {
    if extent > 1e-9 {
        extent * 1.5
    } else {
        20.0
    }
}

/// The axis a vector mostly points along.
fn along_axis(v: Vec3) -> usize {
    if v.x.abs() >= v.y.abs() && v.x.abs() >= v.z.abs() {
        0
    } else if v.y.abs() >= v.z.abs() {
        1
    } else {
        2
    }
}

/// Remove stage `index`, moving later stages up; they then repeat what remains above them. The
/// last stage cannot be removed.
pub fn remove_stage(params: &mut Params, index: usize) {
    let used = stage_count(params);
    if used <= 1 || index >= used {
        return;
    }
    for below in index + 1..used {
        let moved = stage(params, below);
        set_stage(params, below - 1, &moved);
    }
    params.insert("stages".to_string(), ParamValue::Count(used as u32 - 1));
}

/// Swap two stages; the order matters wherever a stage turns or mirrors.
pub fn swap_stages(params: &mut Params, a: usize, b: usize) {
    let used = stage_count(params);
    if a >= used || b >= used || a == b {
        return;
    }
    let (first, second) = (stage(params, a), stage(params, b));
    set_stage(params, a, &second);
    set_stage(params, b, &first);
}
