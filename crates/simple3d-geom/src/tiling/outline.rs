//! The cell outlines a preview draws, before anything is cut.

use super::*;
use crate::vec3::Vec3;

/// The outline of every cell the tiling lays over a shape of these bounds, in
/// the plane the cells tile -- for drawing where the cuts will fall, and for
/// nothing else.
///
/// The layers do not come into it: they cut across the plane rather than within
/// it, so every layer's cells have the one outline.
pub fn cell_outlines(tiling: &Tiling, bounds: (Vec3, Vec3)) -> Vec<Vec<(f64, f64)>> {
    let flat = Tiling { layer: 0.0, ..*tiling };
    if flat.refusal(bounds).is_some() {
        return Vec::new();
    }
    plan(&flat, bounds).into_iter().map(|cell| cell.outline).collect()
}

/// Where the cuts will fall, as loops in the frame the shape's own bounds are
/// given in -- for drawing the tiling over the model itself rather than as a
/// plan beside it (issue 82).
///
/// A cut is a surface, not a line, and drawing every one of them would be a
/// cage nobody can see the shape through. What is drawn instead is the tiling
/// where it meets the shape: at each end of the run along the axis, and at
/// every layer boundary in between. Seen down the axis those coincide and read
/// as one grid, which is the plan; seen from anywhere else they separate, and
/// the separation is what says how deep the cuts go.
///
/// `limit` caps how many loops come back, because ten thousand cells at three
/// depths is thirty thousand outlines and a preview is not worth a frame rate.
/// The ends are laid down before the layers between them, so what survives the
/// cap is the part of the picture that says the most.
pub fn preview_loops(tiling: &Tiling, bounds: (Vec3, Vec3), limit: usize) -> Vec<Vec<Vec3>> {
    let outlines = cell_outlines(tiling, bounds);
    if outlines.is_empty() || limit == 0 {
        return Vec::new();
    }
    let axis = tiling.axis.min(2) as usize;
    let (lo, hi) = ([bounds.0.x, bounds.0.y, bounds.0.z], [bounds.1.x, bounds.1.y, bounds.1.z]);
    let (lo, hi) = (lo[axis], hi[axis]);
    // The far end first, then the near one, then the layers between them: the
    // order the cap eats from the back of.
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
