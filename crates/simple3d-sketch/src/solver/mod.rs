//! Moving a sketch's points until every constraint holds.
//!
//! The unknowns are each free point's X and Y and each circle's radius. Each Newton step changes them
//! as little as it can while making every equation hold to first order, so an under-constrained
//! sketch stays where it was drawn instead of jumping, and a dragged point, weighed heavily, is the
//! one the rest gives way to. Elimination on the Jacobian at the solution says which unknowns the
//! constraints pin down -- what is still free to move is drawn differently -- and which constraints
//! only repeat or contradict others.

mod equations;
pub use equations::angle_between;
mod linalg;
mod system;
mod tangent;

use crate::constraint::{ConstraintId, ConstraintKind};
use crate::curve::CurveId;
use crate::point::PointId;
use crate::sketch::Sketch;
use linalg::{Elimination, Matrix};
use std::collections::HashSet;
use system::{Equation, System};

/// How far off an equation may be and still hold, in millimetres.
pub const TOLERANCE: f64 = 1e-9;

/// Newton steps before giving up; a solvable sketch takes a handful.
const ITERATIONS: usize = 60;

/// How much more a dragged point weighs than the rest.
const DRAGGED: f64 = 1e4;

/// Why a constraint or a value was not taken; the sketch is left as it was.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    /// It cannot be put on what it names.
    Unsupported(&'static str),
    /// It only repeats or contradicts the constraints listed.
    Redundant(Vec<ConstraintId>),
    /// No placement of the points meets it together with the rest.
    Unsolvable,
}

impl Refusal {
    pub fn message(&self) -> String {
        match self {
            Refusal::Unsupported(why) => why.to_string(),
            Refusal::Redundant(_) => "That would over-constrain the sketch: the marked constraints already decide it".into(),
            Refusal::Unsolvable => "The sketch cannot be made to meet that".into(),
        }
    }
}

/// What the constraints leave of the sketch's freedom.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Analysis {
    /// Degrees of freedom left: unknowns the constraints do not decide.
    pub dof: usize,
    /// Points that can still move in some direction.
    pub free_points: HashSet<PointId>,
    /// Curves with any point or radius still free.
    pub free_curves: HashSet<CurveId>,
    /// Constraints repeating or contradicting others.
    pub redundant: HashSet<ConstraintId>,
    /// Whether every equation holds.
    pub solved: bool,
}

fn residuals(sys: &System, eqs: &[Equation], x: &[f64]) -> Vec<f64> {
    eqs.iter().map(|eq| (eq.eval)(sys, x)).collect()
}

fn worst(r: &[f64]) -> f64 {
    r.iter().fold(0.0_f64, |most, v| most.max(v.abs()))
}

/// The Jacobian by central differences, only where an equation reads an unknown.
fn jacobian(sys: &System, eqs: &[Equation], x: &[f64]) -> Matrix {
    let mut x = x.to_vec();
    let mut j = vec![vec![0.0; x.len()]; eqs.len()];
    for (row, eq) in eqs.iter().enumerate() {
        for &k in &eq.deps {
            let keep = x[k];
            let h = 1e-7 * keep.abs().max(1.0);
            x[k] = keep + h;
            let up = (eq.eval)(sys, &x);
            x[k] = keep - h;
            let down = (eq.eval)(sys, &x);
            x[k] = keep;
            j[row][k] = (up - down) / (2.0 * h);
        }
    }
    j
}

/// Newton's method from the sketch's own positions; the unknowns where everything holds, or `None`.
fn newton(sys: &System, eqs: &[Equation], weights: &[f64]) -> Option<Vec<f64>> {
    let mut x = sys.start.clone();
    let norm = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>();
    for _ in 0..ITERATIONS {
        let r = residuals(sys, eqs, &x);
        if worst(&r) < TOLERANCE {
            return Some(x);
        }
        let step = linalg::least_change_step(&jacobian(sys, eqs, &x), &r, weights)?;
        // Halved while it makes things worse, so an overshoot near a fold cannot run away.
        let mut t = 1.0;
        loop {
            let next: Vec<f64> = x.iter().zip(&step).map(|(v, d)| v + t * d).collect();
            if norm(&residuals(sys, eqs, &next)) < norm(&r) || t < 1e-4 {
                x = next;
                break;
            }
            t *= 0.5;
        }
    }
    (worst(&residuals(sys, eqs, &x)) < TOLERANCE).then_some(x)
}

impl Sketch {
    /// Move the points until every constraint holds; `false`, with nothing moved, if they cannot.
    pub fn solve(&mut self) -> bool {
        self.solve_weighted(&[])
    }

    fn solve_weighted(&mut self, heavy: &[PointId]) -> bool {
        match self.solved(heavy) {
            Some(solved) => {
                *self = solved;
                true
            }
            None => false,
        }
    }

    /// A copy with every constraint met, the points in `heavy` moved most reluctantly.
    fn solved(&self, heavy: &[PointId]) -> Option<Sketch> {
        let sys = System::new(self);
        let eqs = sys.equations().ok()?;
        let mut weights = vec![1.0; sys.len()];
        for &point in heavy {
            for k in sys.point_vars(point) {
                weights[k] = DRAGGED;
            }
        }
        let x = newton(&sys, &eqs, &weights)?;
        let mut solved = self.clone();
        sys.write(&x, &mut solved);
        Some(solved)
    }

    /// Move points to where they were dragged and solve, the rest giving way to them. `false`, with
    /// nothing moved, if no placement keeps the constraints.
    pub fn drag(&mut self, moves: &[(PointId, [f64; 2])]) -> bool {
        let before = self.clone();
        for &(id, to) in moves {
            if let Some(point) = self.point_mut(id).filter(|p| !p.fixed) {
                point.at = to;
            }
        }
        let ids: Vec<PointId> = moves.iter().map(|m| m.0).collect();
        if self.solve_weighted(&ids) {
            return true;
        }
        *self = before;
        false
    }

    /// Add a constraint and solve, unless it cannot hold, cannot be met or adds nothing.
    pub fn constrain(&mut self, kind: ConstraintKind) -> Result<ConstraintId, Refusal> {
        let before = self.clone();
        let id = self.push_constraint(kind);
        let refuse = |sketch: &mut Sketch, why: Refusal| {
            *sketch = before.clone();
            Err(why)
        };
        let unsupported = System::new(self).equations().err().map(|(_, why)| why);
        if let Some(why) = unsupported {
            return refuse(self, Refusal::Unsupported(why));
        }
        let solved = self.solve();
        if let Some(others) = self.repeated_by(id) {
            return refuse(self, Refusal::Redundant(others));
        }
        if !solved {
            return refuse(self, Refusal::Unsolvable);
        }
        Ok(id)
    }

    /// Change a dimension's value and solve; refused, with nothing changed, if it cannot be met.
    pub fn set_value(&mut self, id: ConstraintId, value: f64) -> Result<(), Refusal> {
        let before = self.clone();
        let Some(slot) = self.constraint_mut(id).and_then(|c| c.kind.value_mut()) else {
            return Err(Refusal::Unsupported("That constraint has no value"));
        };
        *slot = value;
        if self.solve() {
            return Ok(());
        }
        *self = before;
        Err(Refusal::Unsolvable)
    }

    /// The other constraints constraint `id` only repeats, if it adds nothing of its own.
    fn repeated_by(&self, id: ConstraintId) -> Option<Vec<ConstraintId>> {
        let sys = System::new(self);
        let eqs = sys.equations().ok()?;
        let elimination = Elimination::of(&jacobian(&sys, &eqs, &sys.start));
        let mut others: Vec<ConstraintId> = elimination
            .redundant
            .iter()
            .filter(|(row, _)| eqs[*row].owner == Some(id))
            .flat_map(|(_, made_of)| made_of.iter().filter_map(|&k| eqs[k].owner))
            .filter(|&owner| owner != id)
            .collect();
        let repeats = elimination.redundant.iter().any(|(row, _)| eqs[*row].owner == Some(id));
        others.sort_unstable();
        others.dedup();
        repeats.then_some(others)
    }

    /// What the constraints leave free, and which of them repeat others.
    pub fn analyse(&self) -> Analysis {
        let sys = System::new(self);
        let Ok(eqs) = sys.equations() else { return Analysis::default() };
        let n = sys.len();
        let elimination = Elimination::of(&jacobian(&sys, &eqs, &sys.start));
        let free = |vars: Vec<usize>| vars.into_iter().any(|k| !elimination.pins(k, n));
        let mut analysis = Analysis {
            dof: n - elimination.rank(),
            solved: worst(&residuals(&sys, &eqs, &sys.start)) < 1e3 * TOLERANCE,
            ..Analysis::default()
        };
        for point in &self.points {
            if free(sys.point_vars(point.id)) {
                analysis.free_points.insert(point.id);
            }
        }
        for curve in &self.curves {
            if free(sys.curve_vars(curve.id)) {
                analysis.free_curves.insert(curve.id);
            }
        }
        for (row, made_of) in &elimination.redundant {
            analysis.redundant.extend(eqs[*row].owner);
            analysis.redundant.extend(made_of.iter().filter_map(|&k| eqs[k].owner));
        }
        analysis
    }
}
