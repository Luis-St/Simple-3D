//! The passes one frame is drawn in, and the bias each is given.

use super::*;
use crate::raster::{Rgba, Vertex};

/// The primitives sorted into the passes that draw them, in drawing order.
///
/// A GPU draws in batches, and the batches have to keep the order the software
/// renderer draws in wherever depth does not already decide it: the ground
/// grid goes under the model, the model writes depth and its own body tags,
/// and the ghosts blend over what is there without claiming any of it.
#[derive(Default)]
pub(crate) struct Passes {
    /// Grid lines: blended, depth-tested, and leaving no depth of their own.
    pub(super) grid: Vec<GpuVertex>,
    /// Shaded faces: opaque, writing depth and tag.
    pub(super) solids: Vec<GpuVertex>,
    /// Feature edges, selection outlines, plane marks: opaque lines, biased
    /// towards the eye, writing depth and tag.
    pub(super) lines: Vec<GpuVertex>,
    /// A tool's preview: over the model, blended, depth-tested, and claiming
    /// neither depth nor tag. The same state as the grid and the opposite side
    /// of the model from it, which is the whole reason it is a pass of its own.
    pub(super) overlay: Vec<GpuVertex>,
    /// Ghosts: blended, depth-tested, writing neither depth nor tag.
    pub(super) ghosts: Vec<GpuVertex>,
    /// The glow of a body inside another one: blended over everything, with the
    /// depth test off in both directions.
    pub(super) glow: Vec<GpuVertex>,
    /// The origin axes, with their own rule.
    pub(super) axes: Vec<GpuVertex>,
    /// The smallest and largest depth key in the frame, so the whole scene can
    /// be mapped into the depth buffer's range.
    pub(super) key_range: Option<(f32, f32)>,
}

impl Passes {
    pub(super) fn saw(&mut self, key: f32) {
        self.key_range = Some(match self.key_range {
            None => (key, key),
            Some((lo, hi)) => (lo.min(key), hi.max(key)),
        });
    }

    pub(super) fn triangle(&mut self, v: [Vertex; 3], colour: Rgba, tag: u16, write_depth: bool) {
        let into = if write_depth { &mut self.solids } else { &mut self.ghosts };
        for vertex in v {
            into.push(GpuVertex::new(vertex, colour, tag, 0));
        }
        for vertex in v {
            self.saw(vertex.key);
        }
    }

    pub(super) fn line(&mut self, a: Vertex, b: Vertex, colour: Rgba, bias: f32, tag: u16, write_depth: bool) {
        // The bias is part of the depth the line is tested at, exactly as it is
        // in `Frame::line_inner`, so it is folded into the key here rather than
        // being some separate polygon offset the driver decides the size of.
        let (a, b) = (biased(a, bias), biased(b, bias));
        let into = if write_depth { &mut self.lines } else { &mut self.grid };
        into.push(GpuVertex::new(a, colour, tag, 0));
        into.push(GpuVertex::new(b, colour, tag, 0));
        self.saw(a.key);
        self.saw(b.key);
    }

    pub(super) fn glow(&mut self, v: [Vertex; 3], colour: Rgba) {
        for vertex in v {
            self.glow.push(GpuVertex::new(vertex, colour, 0, 0));
            self.saw(vertex.key);
        }
    }

    pub(super) fn overlay(&mut self, a: Vertex, b: Vertex, colour: Rgba, bias: f32) {
        let (a, b) = (biased(a, bias), biased(b, bias));
        self.overlay.push(GpuVertex::new(a, colour, 0, 0));
        self.overlay.push(GpuVertex::new(b, colour, 0, 0));
        self.saw(a.key);
        self.saw(b.key);
    }
}

pub(crate) fn biased(v: Vertex, bias: f32) -> Vertex {
    Vertex { pos: v.pos, key: v.key + bias }
}

/// The axis bias, matching `render.rs`'s own. Kept in step by the test that
/// draws the same scene through both engines.
pub(crate) const AXIS_BIAS: f32 = -5.0e-4;
