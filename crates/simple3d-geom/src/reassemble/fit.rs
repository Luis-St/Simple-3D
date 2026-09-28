//! Fitting a shape to one body, and measuring whether it is that shape.
//!
//! Fits are closed-form from the body's extents and circles fitted to its end rings; only the
//! direction is searched (see [`super::frame::axes`]). [`measure`] decides between fits by
//! measuring the body's surface against each generated shape, so a bad guess costs only a
//! measurement.

use super::frame::{self, Frame};
use super::*;
use crate::mesh::Mesh;
use crate::simplify::measure;

mod shapes;
use shapes::{as_box, as_round, as_sphere};

/// Work out what one body is; an unrecognised body keeps its triangles.
pub(super) fn part_of(body: &Mesh, plan: &Reassemble, give_up: Abandon<'_>) -> Option<Part> {
    let bounds = crate::aabb::bounds_of(body.positions.iter().copied())?;
    let middle = (bounds.0 + bounds.1) * 0.5;
    let kept = |centre: Vec3, shape: Shape, rotation: Vec3, deviation: f64| Part {
        mesh: body.translated(-centre),
        shape,
        centre,
        rotation,
        deviation,
        bounds,
    };
    if !plan.recognise || body.indices.len() < FEWEST_TRIANGLES {
        return Some(kept(middle, Shape::Mesh, Vec3::ZERO, 0.0));
    }
    match recognise(body, plan.tolerance, give_up)? {
        Some(found) => Some(kept(found.centre, found.shape, found.frame.rotation_deg(), found.deviation)),
        None => Some(kept(middle, Shape::Mesh, Vec3::ZERO, 0.0)),
    }
}

/// A shape fitted to a body, and how far the body is from it.
struct Fitted {
    shape: Shape,
    frame: Frame,
    centre: Vec3,
    deviation: f64,
}

/// The shape a body measures as, or nothing.
///
/// Two passes, since measuring is expensive: a cheap screen over a thinned sample discards
/// clearly wrong fits, then the survivors are measured over every point in preference order.
fn recognise(body: &Mesh, tolerance: f64, give_up: Abandon<'_>) -> Option<Option<Fitted>> {
    let facings = frame::facings(body);
    let mut fits: Vec<Fitted> = Vec::new();
    for axis in frame::axes(body, &facings) {
        if give_up() {
            return None;
        }
        fits.extend(along(body, axis, &facings, tolerance));
    }
    // Many directions yield bit-identical fits (a box from all six normals), so measure each once.
    same_again(&mut fits);
    let samples = samples(body);
    let stride = samples.len().div_ceil(SCREENED).max(1);
    let whole = stride == 1;
    let screen: Vec<Vec3> = samples.iter().copied().step_by(stride).collect();
    for fit in fits.iter_mut() {
        fit.deviation = deviation(&screen, fit);
    }
    // Rank by shape first, then closeness: a cube is exactly both a box and a four-sided prism,
    // and its name must not depend on which of two zeroes is smaller.
    fits.retain(|fit| fit.deviation <= tolerance * SCREEN_SLACK);
    fits.sort_by(|a, b| rank(a.shape).cmp(&rank(b.shape)).then(a.deviation.total_cmp(&b.deviation)));
    // Keep the best few of each kind: otherwise near-identical box fits of a cylinder crowd out
    // the exact cylinder fit, which sorts behind them because boxes are preferred.
    let mut per_kind = [0usize; KINDS];
    fits.retain(|fit| {
        let kind = &mut per_kind[usize::from(rank(fit.shape))];
        *kind += 1;
        *kind <= PER_KIND
    });
    for mut fit in fits.into_iter().take(CONFIRMED) {
        if give_up() {
            return None;
        }
        // Only re-measured if the screen used a thinned sample; otherwise the screen was the measurement.
        if !whole {
            fit.deviation = deviation(&samples, &fit);
        }
        if fit.deviation <= tolerance {
            return Some(Some(fit));
        }
    }
    Some(None)
}

/// Drop duplicate fits: same kind, position and orientation. Parameters follow from those, so
/// they need not be compared.
fn same_again(fits: &mut Vec<Fitted>) {
    let mut kept: Vec<Fitted> = Vec::with_capacity(fits.len());
    for fit in fits.drain(..) {
        let seen = kept.iter().any(|other| {
            rank(other.shape) == rank(fit.shape)
                && (other.centre - fit.centre).length() < SAME_FIT
                && (other.frame.x - fit.frame.x).length() < SAME_FIT
                && (other.frame.z - fit.frame.z).length() < SAME_FIT
        });
        if !seen {
            kept.push(fit);
        }
    }
    *fits = kept;
}

/// The furthest any sample is from the fitted shape, in millimetres.
fn deviation(samples: &[Vec3], fit: &Fitted) -> f64 {
    measure::furthest_from(samples, &fit.shape.mesh().transformed(fit.centre, fit.frame.rotation_deg()))
}

/// The points a fit is measured at: the body's corners and every triangle's middle.
///
/// The middles matter: all corners of a plate with a hole lie on its bounding box, so only the
/// triangles lining the hole reveal it.
fn samples(body: &Mesh) -> Vec<Vec3> {
    let mut out = body.positions.clone();
    out.extend(body.indices.iter().map(|tri| {
        (body.positions[tri[0] as usize] + body.positions[tri[1] as usize] + body.positions[tri[2] as usize]) / 3.0
    }));
    out
}

/// Preference between equally good fits; lower wins.
fn rank(shape: Shape) -> u8 {
    match shape {
        Shape::Box { .. } => 0,
        Shape::Cylinder { .. } => 1,
        Shape::Prism { .. } => 2,
        Shape::Cone { .. } => 3,
        Shape::Sphere { .. } => 4,
        Shape::Mesh => 5,
    }
}

/// Every shape fitted to the body along one direction.
fn along(body: &Mesh, axis: Vec3, facings: &[frame::Facing], tolerance: f64) -> Vec<Fitted> {
    let mut out = Vec::new();
    out.extend(as_box(body, axis, facings));
    out.extend(as_round(body, axis, tolerance));
    out.extend(as_sphere(body, axis));
    out
}

/// The fewest triangles a closed shape can have: a tetrahedron.
const FEWEST_TRIANGLES: usize = 4;

/// How near two fits must stand, in position and direction, to be the same fit.
const SAME_FIT: f64 = 1e-9;

/// How many points a fit is screened over before it is measured in full.
const SCREENED: usize = 1_500;

/// How far past the tolerance a screened fit may measure and still be measured in full. Loose,
/// since a thinned sample can miss the worst spot.
const SCREEN_SLACK: f64 = 8.0;

/// How many screened fits are measured in full; past a handful the body is a mesh.
const CONFIRMED: usize = 12;

/// How many fits of any one shape are measured in full.
const PER_KIND: usize = 2;

/// The number of shape kinds ranked by [`rank`].
const KINDS: usize = 6;
