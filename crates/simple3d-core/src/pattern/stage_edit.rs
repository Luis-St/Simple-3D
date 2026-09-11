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
            let s = above.step;
            let along = if s.x.abs() >= s.y.abs() && s.x.abs() >= s.z.abs() {
                0
            } else if s.y.abs() >= s.z.abs() {
                1
            } else {
                2
            };
            taken[along] = true;
        }
    }
    // Half the shape again, the spacing `params_for_size` gives a new
    // pattern: a visible gap whatever the size.
    let clear = |extent: f64| if extent > 1e-9 { extent * 1.5 } else { 20.0 };
    let extents = [size.x, size.y, size.z];
    match (0..3).find(|axis| !taken[*axis]) {
        Some(axis) => Stage::run(2, unit(axis) * clear(extents[axis])),
        None => Stage::turning(4, 90.0, clear(size.x.max(size.y)) * 2.0, 0.0, 0.0, 2),
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
        set_stage(params, below - 1, moved);
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
    set_stage(params, a, second);
    set_stage(params, b, first);
}
