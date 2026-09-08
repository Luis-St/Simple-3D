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

/// Which of `steps` each band has to draw, as indices into it.
///
/// Sorting the primitives into their bands once beats letting every band walk
/// the whole list: a dense mesh is tens of thousands of primitives and a large
/// frame is fifteen bands, and the scan alone then costs more than the fill.
/// The indices stay ascending, so each band still draws in preparation order.
pub(crate) fn bin_steps(steps: &[Step], ranges: &[(usize, usize)]) -> Vec<Vec<u32>> {
    let mut bins: Vec<Vec<u32>> = ranges.iter().map(|_| Vec::new()).collect();
    for (index, step) in steps.iter().enumerate() {
        let (from, to) = step.rows();
        for (bin, &(lo, hi)) in bins.iter_mut().zip(ranges) {
            // A pixel of slack at each end: a line samples on rounded
            // coordinates and a triangle's span is widened by one, so a
            // primitive that only just misses a band's rows can still write to
            // one of them.
            if to >= lo as f32 - 1.0 && from <= hi as f32 + 1.0 {
                bin.push(index as u32);
            }
        }
    }
    bins
}

/// Draw the prepared primitives listed for this band, in the order they were
/// prepared -- which is the order the single-threaded renderer drew them in.
pub(crate) fn draw_steps(frame: &mut Frame, steps: &[Step], mine: &[u32]) {
    for &index in mine {
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
    let scale = (a.key.abs() + b.key.abs()) * 0.5;
    Step::Line { a, b, colour, bias: bias * scale, tag, write_depth }
}
