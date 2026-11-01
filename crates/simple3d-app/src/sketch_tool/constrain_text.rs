//! How each constraint tool is named, explained, drawn and keyed, and what it asks for next.

use super::constrain::{Counts, Rule};
use crate::icon::Glyph;

impl Rule {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Rule::Coincident => "Coincident",
            Rule::Midpoint => "Midpoint",
            Rule::Horizontal => "Horizontal",
            Rule::Vertical => "Vertical",
            Rule::Parallel => "Parallel",
            Rule::Perpendicular => "Perpendicular",
            Rule::Tangent => "Tangent",
            Rule::Equal => "Equal",
            Rule::Symmetric => "Symmetric",
            Rule::Fix => "Fix",
        }
    }

    pub(crate) fn glyph(self) -> Glyph {
        match self {
            Rule::Coincident => Glyph::ConstrainCoincident,
            Rule::Midpoint => Glyph::ConstrainMidpoint,
            Rule::Horizontal => Glyph::ConstrainHorizontal,
            Rule::Vertical => Glyph::ConstrainVertical,
            Rule::Parallel => Glyph::ConstrainParallel,
            Rule::Perpendicular => Glyph::ConstrainPerpendicular,
            Rule::Tangent => Glyph::ConstrainTangent,
            Rule::Equal => Glyph::ConstrainEqual,
            Rule::Symmetric => Glyph::ConstrainSymmetric,
            Rule::Fix => Glyph::ConstrainFix,
        }
    }

    /// What to select for it.
    pub(crate) fn needs(self) -> &'static str {
        match self {
            Rule::Coincident => "Two points, or a point and the line, arc, circle or ellipse it should lie on",
            Rule::Horizontal | Rule::Vertical => "One or more lines",
            Rule::Parallel | Rule::Perpendicular => "Two lines",
            Rule::Tangent => "Two curves: a line and a round one, two round ones, or two meeting at an end",
            Rule::Equal => "Two or more lines, or two or more arcs and circles",
            Rule::Symmetric => "Two points and the line they mirror across",
            Rule::Midpoint => "A point and a line",
            Rule::Fix => "Points or curves to hold where they are",
        }
    }

    /// What to click, for the tool's hint line.
    pub(crate) fn hint(self) -> &'static str {
        match self {
            Rule::Coincident => "Click two points to join, or a point and the curve it should lie on.",
            Rule::Midpoint => "Click a point and the line it should sit in the middle of.",
            Rule::Horizontal => "Click lines to make them horizontal.",
            Rule::Vertical => "Click lines to make them vertical.",
            Rule::Parallel => "Click two lines to make them parallel.",
            Rule::Perpendicular => "Click two lines to square them to each other.",
            Rule::Tangent => "Click two curves to make them touch smoothly.",
            Rule::Equal => "Click two lines to give them one length, or two arcs or circles for one radius.",
            Rule::Symmetric => "Click two points, and the line they mirror each other across.",
            Rule::Fix => "Click points or curves to hold them where they are.",
        }
    }

    /// What to click next with `picked` so far.
    pub(crate) fn next(self, picked: Counts) -> &'static str {
        let curves = picked.curves();
        match self {
            Rule::Coincident if curves > 0 => "Now click the point to put on it",
            Rule::Coincident => "Now click the point or curve it should meet",
            Rule::Midpoint if picked.points > 0 => "Now click the line",
            Rule::Midpoint => "Now click the point",
            Rule::Parallel | Rule::Perpendicular => "Now click the second line",
            Rule::Tangent => "Now click the second curve",
            Rule::Equal => "Now click the curve to match",
            Rule::Symmetric if picked.points == 0 => "Now click the two points it mirrors",
            Rule::Symmetric if picked.points == 1 && picked.lines == 1 => "Now click the other point",
            Rule::Symmetric if picked.points < 2 => "Now click the second point",
            Rule::Symmetric => "Now click the line they mirror across",
            Rule::Horizontal | Rule::Vertical | Rule::Fix => self.hint(),
        }
    }
}
