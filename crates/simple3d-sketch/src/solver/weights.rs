//! How reluctantly each point moves: a constraint is met by moving the loose ends of the drawing
//! before the points that hold it together, so a line constrained horizontal turns about the end
//! joined to the rest rather than tearing it away. First with the joined points held still, which
//! leaves them exactly where they were; only when that cannot be met do they give way, weighed.

use crate::constraint::ConstraintKind;
use crate::point::PointId;
use crate::sketch::Sketch;
use std::collections::{HashMap, HashSet};

/// How much more a point joined to other geometry weighs than a loose end.
pub(super) const CONNECTED: f64 = 100.0;

impl Sketch {
    /// The points joined to other geometry: shared by two curves or more, or tied to another point
    /// or curve by a coincidence, a point on a curve or a midpoint.
    pub(crate) fn connected_points(&self) -> HashSet<PointId> {
        let mut uses: HashMap<PointId, usize> = HashMap::new();
        for curve in &self.curves {
            let mut points = curve.shape.points();
            points.sort_unstable();
            points.dedup();
            for point in points {
                *uses.entry(point).or_default() += 1;
            }
        }
        let mut connected: HashSet<PointId> = uses.into_iter().filter(|&(_, n)| n > 1).map(|(p, _)| p).collect();
        for constraint in &self.constraints {
            match constraint.kind {
                ConstraintKind::Coincident { a, b } => connected.extend([a, b]),
                ConstraintKind::OnCurve { point, .. } | ConstraintKind::Midpoint { point, .. } => {
                    connected.insert(point);
                }
                _ => {}
            }
        }
        connected
    }

    /// A copy with every constraint met by moving only points outside `connected`, or `None` when
    /// that cannot be done.
    pub(super) fn solved_pinned(&self, connected: &HashSet<PointId>) -> Option<Sketch> {
        let pinned: Vec<PointId> =
            self.points.iter().filter(|p| !p.fixed && connected.contains(&p.id)).map(|p| p.id).collect();
        if pinned.is_empty() {
            return None;
        }
        let mut held = self.clone();
        for &id in &pinned {
            held.point_mut(id)?.fixed = true;
        }
        let mut solved = held.solved_weighted(&[], &HashSet::new())?;
        for &id in &pinned {
            solved.point_mut(id)?.fixed = false;
        }
        Some(solved)
    }
}
