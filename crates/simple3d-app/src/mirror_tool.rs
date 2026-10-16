//! The tool that mirrors objects (issue 122).
//!
//! The objects come first, chosen by selecting them as usual, then the line they are mirrored across:
//! a world axis, a body edge picked in the viewport, or a line drawn there with two clicks, whose ends
//! can be typed. A line alone does not fix a plane, so the mirror's plane holds the line and one more
//! world direction, Z by default: objects mirrored across a line on the grid flip over it as seen from
//! above.
//!
//! Nothing changes until Apply. Meanwhile each object's mirror image is drawn as a translucent template.
//! By default the originals stay and mirrored copies are made beside them. The mirror is carried by a
//! negative scale on one of each object's own axes ([`simple3d_core::scene::Node::mirrored`]), so a
//! mirrored shape keeps its parameters. A non-modal [in-place popup](crate::popup).

mod apply;
mod controls;
mod open;
mod window;
pub(crate) use window::*;
mod pointer;
pub(crate) use pointer::*;
mod overlay;
pub(crate) use overlay::*;
#[cfg(test)]
mod tests;

use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;
use std::hash::{Hash, Hasher};

/// What the objects are mirrored across.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MirrorBy {
    /// A world axis through the origin.
    Axis,
    /// A body edge picked in the viewport.
    Edge,
    /// A line drawn in the viewport, or typed.
    Line,
}

impl MirrorBy {
    pub fn label(self) -> &'static str {
        match self {
            MirrorBy::Axis => "World axis",
            MirrorBy::Edge => "Edge",
            MirrorBy::Line => "Drawn line",
        }
    }
}

/// What the tool is working on while its window is open.
pub struct MirrorTool {
    /// The objects mirrored: the top of the selection.
    pub targets: Vec<NodeId>,
    /// While choosing, the objects follow the selection and the viewport selects as usual; afterwards
    /// its clicks pick the line.
    pub choosing: bool,
    pub by: MirrorBy,
    /// The world axis mirrored across, for [`MirrorBy::Axis`].
    pub axis: usize,
    /// The picked edge's or drawn line's ends; a drawn line fills them one click at a time.
    pub ends: [Option<Vec3>; 2],
    /// The world direction the plane holds beside the line.
    pub upright: usize,
    /// Make mirrored copies and keep the originals, rather than mirror the originals themselves.
    pub keep: bool,
}

impl MirrorTool {
    /// The tool on `targets`, choosing objects first when there are none.
    pub fn on(targets: Vec<NodeId>) -> MirrorTool {
        MirrorTool {
            choosing: targets.is_empty(),
            targets,
            by: MirrorBy::Axis,
            axis: 0,
            ends: [None, None],
            upright: 2,
            keep: true,
        }
    }

    /// Whether the objects are the selection as it changes: while choosing them, and always while
    /// mirroring across an axis, which takes none of the viewport's clicks.
    pub fn follows_selection(&self) -> bool {
        self.choosing || self.by == MirrorBy::Axis
    }

    /// A point on the line and its direction, or what is still missing.
    pub fn line(&self) -> Result<(Vec3, Vec3), String> {
        if self.by == MirrorBy::Axis {
            let mut along = Vec3::ZERO;
            along.set(self.axis, 1.0);
            return Ok((Vec3::ZERO, along));
        }
        let missing = match self.by {
            MirrorBy::Edge => "Click a body edge in the viewport to mirror across.",
            _ => match self.ends[0] {
                Some(_) => "Click the line's end in the viewport, or type it.",
                None => "Click the line's start and end in the viewport.",
            },
        };
        let [Some(start), Some(end)] = self.ends else { return Err(missing.into()) };
        if (end - start).length() <= simple3d_geom::path::SAME_POINT {
            return Err("The line's start and end are the same point".into());
        }
        Ok((start, (end - start).normalized()))
    }

    /// The world direction the plane really holds: the chosen one, or, when the line runs along it,
    /// the one most across the line.
    pub fn holds(&self) -> usize {
        let Ok((_, along)) = self.line() else { return self.upright };
        if along.get(self.upright).abs() < PARALLEL {
            return self.upright;
        }
        (0..3).min_by(|&a, &b| along.get(a).abs().total_cmp(&along.get(b).abs())).unwrap_or(self.upright)
    }

    /// The mirror's plane as a point on it and its unit normal.
    pub fn plane(&self) -> Result<(Vec3, Vec3), String> {
        let (point, along) = self.line()?;
        let mut holds = Vec3::ZERO;
        holds.set(self.holds(), 1.0);
        Ok((point, along.cross(holds).normalized()))
    }

    /// Hash what the templates depend on into the viewport's image key; the objects' own geometry is
    /// covered by the evaluation's generation.
    pub(crate) fn hash_preview<H: Hasher>(&self, hasher: &mut H) {
        (&self.targets, self.choosing, self.by, self.axis, self.upright, self.keep).hash(hasher);
        for point in self.ends.iter().flatten() {
            for number in [point.x, point.y, point.z] {
                number.to_bits().hash(hasher);
            }
        }
    }
}

/// How closely a line may follow the chosen direction before the plane takes another: about 8 degrees.
const PARALLEL: f64 = 0.99;

/// The popup's key, which also remembers where it was dragged.
const KEY: &str = "mirror-tool";

/// The window width: a point's three fields beside their name.
const WIDTH: f32 = 340.0;
