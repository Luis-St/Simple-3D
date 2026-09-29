//! One primitive to rasterise, and the bins they are sorted into.

use super::*;
use crate::raster::{Frame, Rgba, Vertex};
use crate::view::View;
use simple3d_geom::Vec3;

/// One primitive, projected and ready for any band to draw. Band-independent work (projection,
/// culling, shading, line bias) is done once here, or the parallel path would redo it per thread.
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
    /// A line drawn over the model and depth-tested, writing neither depth nor tag: a tool's preview.
    /// Its own variant because the GPU sorts primitives into passes, where a no-depth `Line` meant the
    /// ground grid, drawn under the model.
    Overlay {
        a: Vertex,
        b: Vertex,
        colour: Rgba,
        bias: f32,
    },
    /// A face blended over everything with depth ignored: the glow of a buried body.
    Glow {
        v: [Vertex; 3],
        colour: Rgba,
    },
    /// A line drawn over everything with depth ignored: a tool template's edge, so the whole template
    /// reads as lying on top of a body it overlaps.
    GlowLine {
        a: Vertex,
        b: Vertex,
        colour: Rgba,
    },
}

impl Step {
    /// The rows this primitive can reach, so bands outside them skip it cheaply.
    pub(super) fn rows(&self) -> (f32, f32) {
        match self {
            Step::Triangle { v, .. } => {
                let (a, b, c) = (v[0].pos.y, v[1].pos.y, v[2].pos.y);
                (a.min(b).min(c), a.max(b).max(c))
            }
            Step::Line { a, b, .. } | Step::Overlay { a, b, .. } | Step::GlowLine { a, b, .. } => (a.pos.y.min(b.pos.y), a.pos.y.max(b.pos.y)),
            Step::Glow { v, .. } => {
                let (a, b, c) = (v[0].pos.y, v[1].pos.y, v[2].pos.y);
                (a.min(b).min(c), a.max(b).max(c))
            }
        }
    }
}

/// Which steps each band draws, as index lists per chunk of steps. Binning once beats every band
/// scanning everything; binning itself is parallelised by chunk. Chunk order and ascending indices
/// keep preparation order.
pub(crate) fn bin_steps(steps: &[Step], ranges: &[(usize, usize)]) -> Vec<Vec<Vec<u32>>> {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunks = (steps.len() / 16_384).clamp(1, cores);
    let bin = |from: usize, to: usize| {
        let mut bins: Vec<Vec<u32>> = ranges.iter().map(|_| Vec::new()).collect();
        for (index, step) in steps[from..to].iter().enumerate() {
            let (top, bottom) = step.rows();
            // A pixel of slack each end for rounding and span widening. Ranges are in row order, so the
            // first band is found by binary search.
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

/// The frame cut into `bands` row ranges of about equal work rather than equal height, since the
/// model seldom fills the frame. Cut positions do not affect the picture (see `Frame`). A row's
/// work is the primitives reaching it plus a little for the row itself.
pub(crate) fn balanced_ranges(steps: &[Step], height: usize, bands: usize) -> Vec<(usize, usize)> {
    let mut work = vec![0i64; height + 1];
    // A sample of the steps is enough to locate the work; counting all took longer than drawing a band.
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
    // Start/stop counts to per-row counts to a running total, cutting where it passes each share.
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

/// Draw this band's listed primitives in preparation order, as the single-threaded renderer did.
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
            Step::GlowLine { a, b, colour } => frame.line_over(a, b, colour),
        }
    }
}

/// A world-space line, projected, with its distance-based depth bias; the prepared counterpart
/// of `draw_world_line`.
pub(crate) fn line_step(view: &View, a: Vec3, b: Vec3, colour: Rgba, bias: f32, tag: u16, write_depth: bool) -> Step {
    // No clipping: a parallel projection maps points behind the eye correctly, and the depth key
    // puts them behind everything.
    let (a, b) = (to_vertex(view, view.to_view(a)), to_vertex(view, view.to_view(b)));
    projected_line_step(a, b, colour, bias, tag, write_depth)
}

/// The same, for already projected ends.
pub(crate) fn projected_line_step(a: Vertex, b: Vertex, colour: Rgba, bias: f32, tag: u16, write_depth: bool) -> Step {
    let scale = (a.key.abs() + b.key.abs()) * 0.5;
    Step::Line { a, b, colour, bias: bias * scale, tag, write_depth }
}
