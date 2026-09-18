//! Dropping detail from a mesh while keeping the shape (issue 106).
//!
//! A mesh that came in from a scan or a sculpting tool is often an order of
//! magnitude finer than anything that will be done with it: a hundred thousand
//! triangles describing a bracket whose real shape is a few hundred. The
//! triangles cost time in every boolean it goes through, in every frame it is
//! drawn in, and in the file it is written to, and they are not detail anybody
//! asked for. This takes them out.
//!
//! It works by *edge collapse*, ordered by Garland and Heckbert's quadric error
//! measure: the two ends of an edge are merged into one vertex, the cheapest
//! edge first, where the price of a collapse is how far it moves the surface
//! away from the planes of the triangles that were originally there. Collapsing
//! across a flat face is free however long the edge is, and collapsing across a
//! corner is expensive however short it is -- which is what makes the result
//! look like the shape rather than like a shrunken version of it. See
//! [`Quadric`] for the measure and [`collapse`] for the order it is applied in.
//!
//! ## What it will not do
//!
//! Three things are refused outright, whatever the settings say, because each
//! of them is damage no later collapse can undo:
//!
//! * A collapse that would tear the surface -- see [`collapse::safe`] for the
//!   link condition it is caught by.
//! * A collapse that would turn a triangle inside out.
//! * Any edge where more than two triangles meet, which is where two bodies of
//!   one mesh touch: the seam between the cells of a split is four faces along
//!   one line, and there is no single surface through it to simplify.
//!
//! The rest is settings, and they are all *restrictions*: a boundary to keep, a
//! crease to keep, a colour seam not to collapse across, a distance the surface
//! may not move. See [`Simplify`].

mod quadric;
pub(crate) use quadric::Quadric;
mod surface;
pub(crate) use surface::Surface;
mod collapse;
// Public, and shared with the reassembly (issue 108), which asks the same
// question of a fitted shape that a simplification asks of its result: how far
// is this surface from that one. It is also the one honest way for a test to
// say two meshes describe the same solid.
pub mod measure;
#[cfg(test)]
mod tests;

use crate::mesh::Mesh;
use crate::Abandon;
use serde::{Deserialize, Serialize};

/// How much detail to drop, and what to keep while dropping it (issue 106).
///
/// Every field is a limit on the same one operation, and the two kinds of limit
/// answer different questions. `detail` says *how far to go*: it is a budget,
/// and the collapses stop when it is spent. The rest say *what may not
/// happen on the way*: they are refused collapses, and they can stop the run
/// long before the budget is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Simplify {
    /// How much of the mesh to keep, as a percentage of its triangles. 100
    /// keeps all of them and changes nothing.
    ///
    /// A percentage rather than a triangle count because the same number then
    /// means the same thing on every mesh: "half" is a judgement about how much
    /// detail a shape has to spare, and it carries from one object to the next
    /// in a way "twenty thousand triangles" does not.
    pub detail: u32,
    /// Whether the surface may not move more than `max_deviation` anywhere.
    pub limit_deviation: bool,
    /// How far the surface may move, in millimetres, while `limit_deviation`
    /// is on.
    ///
    /// The guarantee a percentage cannot give. A budget says how many triangles
    /// go; this says what the ones that stay have to be worth, which is the
    /// question a part that has to fit something else is really asking. Kept
    /// through the checkbox being turned off, so the number is still there when
    /// it is turned back on.
    pub max_deviation: f64,
    /// Whether a crease sharper than `sharp_angle` is a feature to keep.
    pub keep_sharp: bool,
    /// How sharp a crease has to be, in degrees, before it counts as a corner
    /// of the shape rather than as a step in how a curve happens to be
    /// tessellated.
    pub sharp_angle: f64,
    /// Whether the rim of a hole stays exactly where it is. A mesh that is not
    /// closed has nothing on the far side of its boundary to hold the surface
    /// in place, so a boundary left free is a boundary that creeps inwards.
    pub keep_boundaries: bool,
    /// Whether the line between two differently painted surfaces stays where it
    /// is. A colour is carried on the triangle (see [`crate::colour_tag`]), so
    /// a collapse across the line between two of them moves the paint as well
    /// as the surface.
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

/// How sharp a crease has to be, by default, to be a corner worth keeping.
///
/// The same judgement the selection outline makes at the same angle: a torus at
/// the stock 32 segments creases at 22.5 degrees around its tube, and calling
/// that a feature would lock every vertex of it and leave nothing to simplify.
/// Thirty-five degrees clears that and still keeps every real corner -- ninety
/// for a box or a cylinder's rim, sixty for a hexagonal prism.
pub const DEFAULT_SHARP: f64 = 35.0;

/// The fewest triangles a simplification will leave.
///
/// Four is a tetrahedron, the smallest closed surface there is. A percentage of
/// a small mesh can ask for fewer, and what comes back from that is not a
/// simplified shape but a handful of triangles that used to be one.
pub const MIN_TRIANGLES: usize = 4;

/// What a simplification produced.
pub struct Outcome {
    pub mesh: Mesh,
    /// The furthest the surface was moved, in millimetres, measured against the
    /// mesh that went in rather than estimated from the collapses -- see
    /// [`measure`].
    ///
    /// What the tool reports, and the one number that makes a simplification
    /// something a part can be trusted to still fit.
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

/// The same, abandoned part-way when `give_up` says the answer is no longer
/// wanted -- the tool asks for a preview on every change of a number, and the
/// one before last is of no interest the moment the next is asked for.
///
/// ## Why the cap can take more than one run
///
/// What the collapses are stopped by is the quadric's own estimate of the
/// error, and what the deviation is finally reported as is a measurement of the
/// result (see [`measure`]). The two do not agree: the estimate is a mean over
/// a vertex's own area and the measurement is the worst point on the surface,
/// so a run stopped at an estimated 0.1 mm can measure 0.3 mm.
///
/// With no cap that discrepancy is only a matter of which number to print, and
/// the printed one is the measured one. With a cap it is the whole point of the
/// setting, so the run is made again with the estimate held proportionally
/// tighter until the measurement is inside what was asked for. It converges in
/// one or two goes -- the two numbers are off by a factor, not by a different
/// shape -- and the attempts are bounded because a tool that sits there
/// halving a threshold is worse than one that keeps slightly more of the mesh
/// than it had to.
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
        // Tightened by how far out the last one was, and then a little further,
        // so an attempt that lands exactly on the cap is not spent finding that
        // out again.
        allowed *= 0.9 * plan.max_deviation / deviation;
    }
    // Every attempt overshot: keep the mesh as it came in rather than a
    // simplification that breaks the promise the cap made.
    Some(Outcome { mesh: welded, deviation: 0.0 })
}

/// How many times a capped run may be tried before it gives up and keeps the
/// mesh whole.
const ATTEMPTS: usize = 4;
