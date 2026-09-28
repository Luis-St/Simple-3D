//! The tool that lines objects up and spreads them out (issue 70).
//!
//! Three ways: align the selection's boxes on an axis by their minimum, centre or maximum; distribute
//! them evenly along an axis; or spread them along a path, picked from body edges or drawn point by
//! point in the viewport. Along a path the count can exceed the selection, and the rest are copies
//! made in the same groups as their originals.
//!
//! Nothing changes until Apply. Meanwhile every object that would move, and every copy that would be
//! made, is drawn as a translucent template where it would land ([`templates`]), so the result can be
//! judged on the model. A non-modal [in-place popup](crate::popup); while spreading along a path, the
//! viewport's clicks build the path.

mod plan;
pub(crate) use plan::*;
mod apply;
mod open;
mod window;
pub(crate) use window::*;
mod controls;
use controls::*;
mod path_controls;
use path_controls::*;
mod pointer;
pub(crate) use pointer::*;
mod overlay;
pub(crate) use overlay::*;
#[cfg(test)]
mod tests;

use simple3d_core::scene::NodeId;
use simple3d_geom::path::Path;
use simple3d_geom::Vec3;
use std::hash::{Hash, Hasher};

/// What the tool does with the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Arrange {
    Align,
    Distribute,
    Path,
}

impl Arrange {
    pub fn label(self) -> &'static str {
        match self {
            Arrange::Align => "Align",
            Arrange::Distribute => "Distribute",
            Arrange::Path => "Along a path",
        }
    }
}

/// Which side of a box lines up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Min,
    Centre,
    Max,
}

impl Side {
    pub const ALL: [Side; 3] = [Side::Min, Side::Centre, Side::Max];

    pub fn label(self) -> &'static str {
        match self {
            Side::Min => "Min",
            Side::Centre => "Centre",
            Side::Max => "Max",
        }
    }

    /// Where this side of `bounds` is on `axis`.
    pub fn of(self, (lo, hi): (Vec3, Vec3), axis: usize) -> f64 {
        match self {
            Side::Min => lo.get(axis),
            Side::Centre => (lo.get(axis) + hi.get(axis)) / 2.0,
            Side::Max => hi.get(axis),
        }
    }
}

/// How distributed objects are spaced along the axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Spacing {
    /// The outer two stay; the space between neighbours is the same.
    Gaps,
    /// The outer two stay; the centres are the same distance apart.
    Centres,
    /// The first stays; each next one follows a set gap after the one before.
    Fixed,
}

/// Where the path comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathSource {
    /// Picked body edges, chained in the order they connect.
    Edges,
    /// Points clicked in the viewport, joined by straight lines.
    Drawn,
}

/// What the tool is working on while its window is open.
pub struct ArrangeTool {
    /// The objects it arranges: the top of the selection it opened on, in tree order.
    pub targets: Vec<NodeId>,
    /// The last selected of them, which "Relative to" can hold still.
    pub key: Option<NodeId>,
    pub mode: Arrange,
    /// Per axis, the side lined up, or none to leave that axis alone.
    pub align: [Option<Side>; 3],
    /// Align on the key object rather than on the box around all of them.
    pub to_key: bool,
    /// The axis objects are distributed along.
    pub axis: usize,
    pub spacing: Spacing,
    /// The gap for [`Spacing::Fixed`], in millimetres.
    pub gap: f64,
    pub source: PathSource,
    /// Picked edges, one run per click, so a right-click takes a whole click back.
    pub runs: Vec<Vec<(Vec3, Vec3)>>,
    /// Whether a click on an edge takes the smooth run it belongs to, such as a whole round rim.
    pub whole_run: bool,
    pub points: Vec<Vec3>,
    /// A drawn path runs back to its first point.
    pub closed: bool,
    /// How many objects go along the path; beyond the selection, copies.
    pub count: usize,
    /// Turn each object about Z as the path turns.
    pub follow: bool,
}

impl ArrangeTool {
    /// The path objects are spread along.
    pub fn path(&self) -> Path {
        match self.source {
            PathSource::Edges => Path::from_edges(&self.runs.concat()),
            PathSource::Drawn => Path { points: self.points.clone(), closed: self.closed && self.points.len() > 2 },
        }
    }

    /// Hash what the templates depend on into the viewport's image key; the objects' own geometry is
    /// covered by the evaluation's generation.
    pub(crate) fn hash_preview<H: Hasher>(&self, hasher: &mut H) {
        (&self.targets, self.key, self.mode, self.align, self.to_key, self.axis, self.spacing).hash(hasher);
        (self.source, self.closed, self.count, self.follow, self.gap.to_bits()).hash(hasher);
        for point in self.runs.iter().flatten().flat_map(|&(a, b)| [a, b]).chain(self.points.iter().copied()) {
            for number in [point.x, point.y, point.z] {
                number.to_bits().hash(hasher);
            }
        }
    }
}

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "arrange-tool";

/// The window width: three side chips beside an axis name, and a point's three fields.
const WIDTH: f32 = 350.0;

/// A number field's width: enough for a length with its unit.
const FIELD_WIDTH: f32 = 90.0;

/// The most objects one spread along a path makes, so a slip of the count cannot freeze the model.
const MOST_COUNT: u32 = 500;
