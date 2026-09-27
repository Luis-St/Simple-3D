//! Dropping detail from a mesh while keeping the shape (issue 106).
//!
//! Edge collapse ordered by Garland and Heckbert's quadric error: the cheapest collapse moves the
//! surface least from the original planes, so flat faces collapse freely and corners resist. See
//! [`Quadric`] and [`collapse`].
//!
//! Always refused, since no later collapse can undo them: tearing the surface
//! ([`collapse::safe`]), inverting a triangle, and collapsing edges where more than two triangles
//! meet (touching bodies). Everything else is a restriction set in [`Simplify`].

mod quadric;
pub(crate) use quadric::Quadric;
mod surface;
pub(crate) use surface::Surface;
mod collapse;
// Public and shared with the reassembly (issue 108): the distance between two surfaces.
pub mod measure;
#[cfg(test)]
mod tests;

use crate::mesh::Mesh;
use crate::Abandon;
use serde::{Deserialize, Serialize};

/// How much detail to drop, and what to keep while dropping it (issue 106). `detail` is a budget;
/// the rest refuse collapses and may stop the run earlier.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Simplify {
    /// The percentage of triangles to keep; 100 changes nothing. A percentage carries across meshes
    /// in a way a triangle count does not.
    pub detail: u32,
    /// Whether the surface may not move more than `max_deviation` anywhere.
    pub limit_deviation: bool,
    /// How far the surface may move, in millimetres, while `limit_deviation` is on. Kept while off.
    pub max_deviation: f64,
    /// Whether a crease sharper than `sharp_angle` is a feature to keep.
    pub keep_sharp: bool,
    /// How sharp a crease must be, in degrees, to count as a corner rather than tessellation.
    pub sharp_angle: f64,
    /// Whether a hole's rim stays in place; a free boundary creeps inwards.
    pub keep_boundaries: bool,
    /// Whether the line between differently painted surfaces stays in place, since colour is carried
    /// per triangle ([`crate::colour_tag`]).
    pub keep_colours: bool,
}

impl Default for Simplify {
    fn default() -> Simplify {
        Simplify {
            detail: 50,
            limit_deviation: false,
            max_deviation: 0.1,
            keep_sharp: true,
            sharp_angle: DEFAULT_SHARP,
            keep_boundaries: true,
            keep_colours: true,
        }
    }
}

/// The default corner angle. A 32-segment torus creases at 22.5 degrees, which must not lock it;
/// 35 still keeps every real corner.
pub const DEFAULT_SHARP: f64 = 35.0;

/// The fewest triangles a simplification leaves: a tetrahedron.
pub const MIN_TRIANGLES: usize = 4;

/// What a simplification produced.
pub struct Outcome {
    pub mesh: Mesh,
    /// The furthest the surface moved, in millimetres, measured against the input ([`measure`]).
    pub deviation: f64,
}

impl Simplify {
    /// How many triangles the budget leaves, for a mesh of `triangles`.
    pub fn target(&self, triangles: usize) -> usize {
        let kept = (triangles as f64 * f64::from(self.detail.min(100)) / 100.0).round() as usize;
        kept.clamp(MIN_TRIANGLES.min(triangles), triangles)
    }
}

/// Simplify a mesh. See [`Simplify`] for what the settings mean.
pub fn simplify(mesh: &Mesh, plan: &Simplify) -> Outcome {
    simplify_until(mesh, plan, &crate::never).expect("a run that is never abandoned finishes")
}

/// The same, abandoned when `give_up` says the answer is no longer wanted.
///
/// With a deviation cap, collapses are stopped by the quadric's mean estimate while the result is
/// measured at its worst point, so the two disagree. The run is repeated with a tighter estimate
/// until the measurement fits; it converges in one or two tries and is bounded by [`ATTEMPTS`].
pub fn simplify_until(mesh: &Mesh, plan: &Simplify, give_up: Abandon<'_>) -> Option<Outcome> {
    let welded = mesh.weld();
    let mut allowed = plan.max_deviation;
    for _ in 0..ATTEMPTS {
        let attempt = Simplify { max_deviation: allowed, ..*plan };
        let mut surface = Surface::build(&welded, &attempt);
        let target = attempt.target(surface.live);
        collapse::run(&mut surface, &attempt, target, give_up)?;
        let result = surface.finish();
        let deviation = measure::furthest_from(&welded.positions, &result);
        if !plan.limit_deviation || deviation <= plan.max_deviation || deviation <= 0.0 {
            return Some(Outcome { mesh: result, deviation });
        }
        // Tightened by the overshoot and a bit more, so a try landing exactly on the cap is not repeated.
        allowed *= 0.9 * plan.max_deviation / deviation;
    }
    // Every attempt overshot: keep the input rather than break the cap.
    Some(Outcome { mesh: welded, deviation: 0.0 })
}

/// How many capped runs are tried before keeping the mesh whole.
const ATTEMPTS: usize = 4;
