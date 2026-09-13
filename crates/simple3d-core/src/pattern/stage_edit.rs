//! Adding, dropping and reordering the stages of a custom rule (issue 79).

use super::*;
use crate::primitive::{ParamValue, Params};
use simple3d_geom::Vec3;

/// The numbers a stage that has just been added starts with, given the stages
/// above it and the size of what the pattern repeats.
///
/// A new stage used to be whatever its slot happened to hold: stage 4's stock
/// turn after a grid, one copy in place after a blank rule -- and in the second
/// case adding a stage appeared to do nothing at all. It now starts as the next
/// thing the rule is missing: a run along the first axis no run above it goes
/// along yet, spaced to clear the shape, and once all three are taken, a ring.
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

/// The numbers a stage that has just been added starts with when what it is to
/// do was chosen with it -- the tool's "Add a stage" offers the three modes
/// rather than one button (issue 79).
///
/// A run is what [`fresh_stage`] would make, along an axis nothing above it
/// runs along yet, or along X once all three are taken; a turn is a ring round
/// the shape; a mirror reflects across X, beside the copies the stages above
/// it laid out along that axis rather than on top of them.
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

/// The variation a stage is given when one of `what` is added to it (issue
/// 79): something visible, sized to the shape and to the stage it is on, so
/// adding one shows what it does before any number is typed.
///
/// A shift staggers: every other copy moved on by half the step of the nearest
/// run above -- the brick bond -- or, with no run above to stagger against, a
/// zigzag across the stage's own run, half the shape wide. A spin builds up a
/// fifteenth of a right angle a copy, a size a tenth smaller a copy, and a gap
/// a quarter of the step wider a copy.
///
/// Where the stage already holds one just like it, the nearest one that is not
/// -- another axis, or other copies -- so the chip adds a variation rather than
/// a second copy of one. `None` where the stage has every one there is.
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

/// Put `variation` on the end of stage `index`'s list. False, and nothing
/// written, where the stage already holds one just like it (see
/// [`Variation::combination`]).
pub fn add_variation(params: &mut Params, index: usize, variation: Variation) -> bool {
    let stage = stage(params, index);
    if stage.variations().iter().any(|held| held.combination() == variation.combination()) {
        return false;
    }
    set_stage(params, index, &stage.with(variation));
    true
}

/// Take variation `slot` off stage `index`; the ones after it move up, and go
/// on being applied in the order they were.
pub fn remove_variation(params: &mut Params, index: usize, slot: usize) {
    let mut stage = stage(params, index);
    if slot >= stage.vary.len() {
        return;
    }
    stage.vary.remove(slot);
    set_stage(params, index, &stage);
}

/// A ring round the shape: four copies a right angle apart, far enough out to
/// stand clear of each other.
fn fresh_turn(size: Vec3) -> Stage {
    Stage::turning(4, 90.0, clear(size.x.max(size.y)) * 2.0, 0.0, 0.0, 2)
}

/// Half the shape again, the spacing `params_for_size` gives a new pattern: a
/// visible gap whatever the size.
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

/// Take stage `index` out of the rule, and move every stage below it up one.
///
/// A stage below the one that goes then repeats what the stages above *it*
/// still make -- which is well defined, and is exactly what dropping a stage
/// from the middle of a stack means. The last one cannot go: a rule is at least
/// one stage.
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

/// Swap two stages. The order only matters where a stage turns or mirrors --
/// two runs give the same copies either way round -- and there it is the whole
/// difference between a ring of rows and a row of rings.
pub fn swap_stages(params: &mut Params, a: usize, b: usize) {
    let used = stage_count(params);
    if a >= used || b >= used || a == b {
        return;
    }
    let (first, second) = (stage(params, a), stage(params, b));
    set_stage(params, a, &second);
    set_stage(params, b, &first);
}
