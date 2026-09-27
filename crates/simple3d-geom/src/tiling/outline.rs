//! The cell outlines a preview draws, before anything is cut.

use super::*;
use crate::vec3::Vec3;

/// Every cell outline the tiling lays over a shape of these bounds, in the cell plane, for drawing.
/// Layers share the same outlines.
pub fn cell_outlines(tiling: &Tiling, bounds: (Vec3, Vec3)) -> Vec<Vec<(f64, f64)>> {
    let flat = Tiling { layer: 0.0, ..*tiling };
    if flat.refusal(bounds).is_some() {
        return Vec::new();
    }
    plan(&flat, bounds).into_iter().map(|cell| cell.outline).collect()
}

/// Where the cuts fall, as loops in the bounds' frame, for drawing on the model (issue 82): the
/// tiling at each end of the run and every layer boundary, which read as one grid down the axis and
/// show cut depth from elsewhere. `limit` caps the loops; ends come first so they survive it.
pub fn preview_loops(tiling: &Tiling, bounds: (Vec3, Vec3), limit: usize) -> Vec<Vec<Vec3>> {
    let outlines = cell_outlines(tiling, bounds);
    if outlines.is_empty() || limit == 0 {
        return Vec::new();
    }
    let axis = tiling.axis.min(2) as usize;
    let (lo, hi) = ([bounds.0.x, bounds.0.y, bounds.0.z], [bounds.1.x, bounds.1.y, bounds.1.z]);
    let (lo, hi) = (lo[axis], hi[axis]);
    // Far end, near end, then the layers: the cap trims from the back.
    let mut depths = vec![hi, lo];
    if tiling.layer >= MIN_SIZE {
        for k in 1..tiling.layer_count(hi - lo) {
            let at = lo + k as f64 * tiling.layer;
            if at > lo && at < hi {
                depths.push(at);
            }
        }
    }
    let mut loops = Vec::new();
    for depth in depths {
        if loops.len() >= limit {
            break;
        }
        for outline in &outlines {
            if loops.len() >= limit {
                break;
            }
            loops.push(outline.iter().map(|&(u, v)| tiling.to_world(u, v, depth)).collect());
        }
    }
    loops
}
