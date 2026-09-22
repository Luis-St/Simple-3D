//! One primitive to rasterise, and the bins they are sorted into.

use super::*;
use crate::raster::{Frame, Rgba, Vertex};
use crate::view::View;
use simple3d_geom::Vec3;

/// One primitive of the model, already projected into screen space and ready
/// for any band of the frame to draw.
///
/// Everything that does not depend on which rows are being drawn -- projection,
/// back-face culling, shading, the depth bias a line gets from its own distance
/// -- is worked out once here rather than once per band. Without that the
/// parallel path re-derives the whole model for every thread, and a dense mesh
/// in a small viewport comes out *slower* than drawing it on one core.
#[derive(Clone, Copy)]
pub(crate) enum Step {
    Triangle {
        v: [Vertex; 3],
        colour: Rgba,
        tag: u16,
        write_depth: bool,
    },
    Line {
        a: Vertex,
        b: Vertex,
        colour: Rgba,
        bias: f32,
        tag: u16,
        write_depth: bool,
    },
    /// A line drawn *over* the model and tested against it, claiming neither
    /// the depth of a pixel nor its body: a tool's preview, which has to be
    /// hidden by the solid it is drawn on the far side of and must not hide the
    /// next loop of itself where two of them cross.
    ///
    /// Its own variant rather than a `Line` that writes no depth, because the
    /// two are told apart by *when* they are drawn and only the software
    /// renderer keeps the order it was handed: the GPU sorts the primitives
    /// into passes, and a line that writes no depth used to mean the ground
    /// grid, which goes under the model.
    Overlay {
        a: Vertex,
        b: Vertex,
        colour: Rgba,
        bias: f32,
    },
    /// A face blended over whatever is already drawn, depth ignored both ways:
    /// the glow of a body inside another one.
    Glow {
        v: [Vertex; 3],
        colour: Rgba,
    },
}

impl Step {
    /// The rows this primitive can reach. A band sharing none of them skips it
    /// on one comparison, which is what the split is worth.
    pub(super) fn rows(&self) -> (f32, f32) {
        match self {
            Step::Triangle { v, .. } => {
                let (a, b, c) = (v[0].pos.y, v[1].pos.y, v[2].pos.y);
                (a.min(b).min(c), a.max(b).max(c))
            }
            Step::Line { a, b, .. } | Step::Overlay { a, b, .. } => (a.pos.y.min(b.pos.y), a.pos.y.max(b.pos.y)),
            Step::Glow { v, .. } => {
                let (a, b, c) = (v[0].pos.y, v[1].pos.y, v[2].pos.y);
                (a.min(b).min(c), a.max(b).max(c))
            }
        }
    }
}

/// Which of `steps` each band has to draw, as indices into it: for each of a
/// run of consecutive chunks of the steps, one list per band.
///
/// Sorting the primitives into their bands once beats letting every band walk
/// the whole list: a dense mesh is hundreds of thousands of primitives and a
/// large frame is a band per core, and the scan alone then costs more than the
/// fill. The sorting is shared out in chunks, because on one thread it took
/// longer than every band's drawing together. A band draws its lists in chunk
/// order, and within one the indices ascend, so it still draws in preparation
/// order.
pub(crate) fn bin_steps(steps: &[Step], ranges: &[(usize, usize)]) -> Vec<Vec<Vec<u32>>> {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunks = (steps.len() / 16_384).clamp(1, cores);
    let bin = |from: usize, to: usize| {
        let mut bins: Vec<Vec<u32>> = ranges.iter().map(|_| Vec::new()).collect();
        for (index, step) in steps[from..to].iter().enumerate() {
            let (top, bottom) = step.rows();
            // A pixel of slack at each end: a line samples on rounded
            // coordinates and a triangle's span is widened by one, so a
            // primitive that only just misses a band's rows can still write to
            // one of them.
            //
            // The ranges are in row order, so the first band the primitive can
            // reach is found by halving rather than by asking every band.
            let first = ranges.partition_point(|&(_, hi)| (hi as f32 + 1.0) < top);
            for (bin, &(lo, _)) in bins[first..].iter_mut().zip(&ranges[first..]) {
                if bottom < lo as f32 - 1.0 {
                    break;
                }
                bin.push((from + index) as u32);
            }
        }
        bins
    };
    if chunks == 1 {
        return vec![bin(0, steps.len())];
    }
    std::thread::scope(|scope| {
        let bin = &bin;
        let handles: Vec<_> = (0..chunks)
            .map(|i| scope.spawn(move || bin(steps.len() * i / chunks, steps.len() * (i + 1) / chunks)))
            .collect();
        handles.into_iter().map(|handle| handle.join().expect("a binning thread panicked")).collect()
    })
}

/// The frame cut into `bands` stretches of rows holding about the same amount
/// of work each, rather than the same number of rows.
///
/// A model seldom fills the frame: cut into equal heights, the bands across
/// the middle of it had all the triangles and the rest had the background,
/// and the frame took as long as its busiest band. Where the cuts fall does
/// not change the picture -- see `Frame` -- so they can go wherever the work
/// is. The work of a row is counted as the primitives that reach it, plus a
/// little for the row itself, so an empty stretch of sky is not all given to
/// one band.
pub(crate) fn balanced_ranges(steps: &[Step], height: usize, bands: usize) -> Vec<(usize, usize)> {
    let mut work = vec![0i64; height + 1];
    // A sample of the steps is as good a guide to where the work is as all of
    // them, and counting all of a dense mesh's took longer than drawing a band.
    let stride = (steps.len() / 65_536).max(1);
    for step in steps.iter().step_by(stride) {
        let (from, to) = step.rows();
        let from = from.max(0.0).min(height as f32) as usize;
        let to = (to.max(0.0) as usize + 1).min(height);
        if from < to {
            work[from] += 1;
            work[to] -= 1;
        }
    }
    // From "starts here" and "stops here" to the count per row, and then a
    // running total, so each cut is where the total passes its share.
    let mut running = 0i64;
    let mut total = 0u64;
    let per_row: Vec<u64> = (0..height)
        .map(|row| {
            running += work[row];
            let row_work = running.max(0) as u64 + 4;
            total += row_work;
            total
        })
        .collect();
    let mut ranges = Vec::with_capacity(bands);
    let mut lo = 0;
    for band in 1..=bands {
        let hi = match band {
            b if b == bands => height,
            b => {
                let share = total * b as u64 / bands as u64;
                per_row.partition_point(|&sum| sum < share).clamp(lo, height)
            }
        };
        if hi > lo {
            ranges.push((lo, hi));
            lo = hi;
        }
    }
    ranges
}

/// Draw the prepared primitives listed for this band, in the order they were
/// prepared -- which is the order the single-threaded renderer drew them in.
pub(crate) fn draw_steps(frame: &mut Frame, steps: &[Step], mine: &[&[u32]]) {
    for &index in mine.iter().flat_map(|part| part.iter()) {
        match steps[index as usize] {
            Step::Triangle { v, colour, tag, write_depth } => {
                frame.set_tag(tag);
                frame.triangle(v, colour, write_depth);
            }
            Step::Line { a, b, colour, bias, tag, write_depth } => {
                frame.set_tag(tag);
                if write_depth {
                    frame.line(a, b, colour, bias);
                } else {
                    frame.line_with_depth(a, b, colour, bias, false);
                }
            }
            Step::Overlay { a, b, colour, bias } => {
                frame.set_tag(0);
                frame.line_with_depth(a, b, colour, bias, false);
            }
            Step::Glow { v, colour } => frame.triangle_over(v, colour),
        }
    }
}

/// A world-space line, projected and given the depth bias its own distance
/// earns it. The counterpart of `draw_world_line`, for the prepared path.
pub(crate) fn line_step(view: &View, a: Vec3, b: Vec3, colour: Rgba, bias: f32, tag: u16, write_depth: bool) -> Step {
    // Nothing is clipped: a parallel projection maps a point behind the eye to
    // its true screen position, and the depth key puts it behind everything
    // else on its own.
    let (a, b) = (to_vertex(view, view.to_view(a)), to_vertex(view, view.to_view(b)));
    projected_line_step(a, b, colour, bias, tag, write_depth)
}

/// The same, for two ends that have been projected already.
pub(crate) fn projected_line_step(a: Vertex, b: Vertex, colour: Rgba, bias: f32, tag: u16, write_depth: bool) -> Step {
    let scale = (a.key.abs() + b.key.abs()) * 0.5;
    Step::Line { a, b, colour, bias: bias * scale, tag, write_depth }
}
