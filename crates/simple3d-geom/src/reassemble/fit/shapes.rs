//! The closed-form fits: box, round (prism, cylinder, cone) and sphere, and the rings they read.

use super::*;

/// The body as the smallest box along `axis`, oriented by the largest face square to it.
pub(super) fn as_box(body: &Mesh, axis: Vec3, facings: &[frame::Facing]) -> Option<Fitted> {
    // The second direction comes from a face, or the box would be turned by an arbitrary amount.
    let square = facings
        .iter()
        .take(FACINGS_SQUARE)
        .find(|facing| facing.normal.dot(axis).abs() < SQUARE_ENOUGH)
        .map_or(Vec3::ZERO, |facing| facing.normal);
    let frame = frame::squared_to_world(Frame::new(axis, square));
    let local: Vec<Vec3> = body.positions.iter().map(|&p| frame.local(p)).collect();
    let (lo, hi) = crate::aabb::bounds_of(local.iter().copied())?;
    let size = hi - lo;
    (size.x > 0.0 && size.y > 0.0 && size.z > 0.0).then_some(Fitted {
        shape: Shape::Box { width: size.x, depth: size.y, height: size.z },
        frame,
        centre: frame.world((lo + hi) * 0.5),
        deviation: f64::MAX,
    })
}

/// The body as a regular polygon extruded along `axis`: a cylinder, prism or cone. All three
/// share one mesh layout (a ring at each end) and are told apart by radii and side count.
pub(super) fn as_round(body: &Mesh, axis: Vec3, tolerance: f64) -> Option<Fitted> {
    let along: Vec<f64> = body.positions.iter().map(|&p| p.dot(axis)).collect();
    let (low, high) = (along.iter().copied().fold(f64::MAX, f64::min), along.iter().copied().fold(f64::MIN, f64::max));
    let height = high - low;
    if height <= 0.0 {
        return None;
    }
    let slack = tolerance.min(height / 4.0);
    let mut bottom = Vec::new();
    let mut top = Vec::new();
    for (&p, &z) in body.positions.iter().zip(along.iter()) {
        if z - low <= slack {
            bottom.push(p);
        } else if high - z <= slack {
            top.push(p);
        } else {
            // A vertex between the ends rules this fit out; the cheapest and most common exit, and what
            // keeps spheres from running through all of it.
            return None;
        }
    }
    let plain = Frame::new(axis, Vec3::ZERO);
    let (bottom_centre, bottom_radius) = ring(&bottom, &plain)?;
    let (top_centre, top_radius) = ring(&top, &plain)?;
    let (wider, wider_radius) = if bottom.len() >= top.len() { (&bottom, bottom_radius) } else { (&top, top_radius) };
    let (sides, phase) = spokes(wider, &plain, (bottom_centre + top_centre) * 0.5, wider_radius)?;
    let frame = plain.turned(phase);
    // The axis runs through the middle of the two rings; the shape's centre is halfway up it.
    let perpendicular = (bottom_centre + top_centre) * 0.5;
    let centre = perpendicular - axis * perpendicular.dot(axis) + axis * ((low + high) / 2.0);
    let shape = if (bottom_radius - top_radius).abs() > tolerance {
        Shape::Cone { bottom_diameter: bottom_radius * 2.0, top_diameter: top_radius * 2.0, height, segments: sides }
    } else {
        // Both rings used, halving what a misplaced vertex can do to the diameter.
        let diameter = bottom_radius + top_radius;
        if sides >= ROUND_SIDES {
            Shape::Cylinder { diameter, height, segments: sides }
        } else {
            Shape::Prism { sides, diameter, height }
        }
    };
    Some(Fitted { shape, frame, centre, deviation: f64::MAX })
}

/// The body as an ellipsoid with `axis` through its poles.
pub(super) fn as_sphere(body: &Mesh, axis: Vec3) -> Option<Fitted> {
    let count = body.positions.len();
    if count < 8 {
        return None;
    }
    let centre = body.positions.iter().fold(Vec3::ZERO, |sum, &p| sum + p) / count as f64;
    let plain = Frame::new(axis, Vec3::ZERO);
    // How far the widest part stands off the axis, which tells a pole from a vertex near one.
    let radius =
        body.positions.iter().map(|&p| (p - centre).dot(plain.x).hypot((p - centre).dot(plain.y))).fold(0.0, f64::max);
    let (sides, phase) = spokes(&body.positions, &plain, centre, radius)?;
    // A tessellated ellipsoid has an exact vertex count for its meridians. Checking it here avoids
    // building and measuring a huge sphere against, say, a 64-segment cylinder.
    let bands = (sides / 2).max(2) as usize;
    if count != sides as usize * (bands - 1) + 2 {
        return None;
    }
    let frame = plain.turned(phase);
    let local: Vec<Vec3> = body.positions.iter().map(|&p| frame.local(p - centre)).collect();
    let (lo, hi) = crate::aabb::bounds_of(local.iter().copied())?;
    let size = hi - lo;
    (size.x > 0.0 && size.y > 0.0 && size.z > 0.0).then_some(Fitted {
        shape: Shape::Sphere { diameter_x: size.x, diameter_y: size.y, diameter_z: size.z, segments: sides },
        frame,
        centre,
        deviation: f64::MAX,
    })
}

/// The circle a ring of vertices sits on, square to the frame's axis: centre and radius.
///
/// Least squares rather than extents, since extents of a polygon (a triangular prism) give the
/// wrong centre and radius. Fitted twice, the second time without points near the centre: a cap
/// fan's centre vertex (`flat_cap`) otherwise pulls the radius in by about a percent, enough to
/// stop a plain cylinder being recognised.
fn ring(points: &[Vec3], frame: &Frame) -> Option<(Vec3, f64)> {
    let (middle, radius) = circle(points, frame)?;
    if radius <= 0.0 {
        return Some((middle, radius));
    }
    let rim: Vec<Vec3> = points
        .iter()
        .copied()
        .filter(|&p| (p - middle).dot(frame.x).hypot((p - middle).dot(frame.y)) > radius / 2.0)
        .collect();
    if rim.len() == points.len() {
        return Some((middle, radius));
    }
    circle(&rim, frame).or(Some((middle, radius)))
}

/// The circle through a set of points, by algebraic least squares. Fewer than three points are
/// taken as an end closed to a point, like a cone's apex.
fn circle(points: &[Vec3], frame: &Frame) -> Option<(Vec3, f64)> {
    let count = points.len();
    if count == 0 {
        return None;
    }
    let middle = points.iter().fold(Vec3::ZERO, |sum, &p| sum + p) / count as f64;
    if count < 3 {
        return Some((middle, 0.0));
    }
    let flat: Vec<(f64, f64)> =
        points.iter().map(|&p| ((p - middle).dot(frame.x), (p - middle).dot(frame.y))).collect();
    let (mut suu, mut svv, mut suv, mut sz, mut suz, mut svz) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for &(u, v) in &flat {
        let z = u * u + v * v;
        suu += u * u;
        svv += v * v;
        suv += u * v;
        sz += z;
        suz += u * z;
        svz += v * z;
    }
    let det = suu * svv - suv * suv;
    if det.abs() < 1e-12 {
        // All points collinear: a two-vertex ring, or no ring around this axis.
        return None;
    }
    let (d, e) = ((-suz * svv + svz * suv) / det, (-svz * suu + suz * suv) / det);
    let f = -sz / count as f64;
    let (cu, cv) = (-d / 2.0, -e / 2.0);
    let radius = (cu * cu + cv * cv - f).max(0.0).sqrt();
    Some((middle + frame.x * cu + frame.y * cv, radius))
}

/// How many evenly spaced directions a ring stands in around `centre`, and the first one's
/// turn from the frame's X: the shape's segments and phase. Both must be right, or the fit is
/// a different solid.
fn spokes(points: &[Vec3], frame: &Frame, centre: Vec3, radius: f64) -> Option<(u32, f64)> {
    // Relative to the ring's radius rather than absolute: a cap's centre vertex stored as `f32` in
    // a project file is slightly off-axis, and an absolute threshold counted it as an extra side
    // (a hexagonal prism became seven-sided).
    let inside = (radius / 2.0).max(ON_AXIS);
    let mut angles: Vec<f64> = Vec::new();
    for &p in points {
        let from = p - centre;
        let (u, v) = (from.dot(frame.x), from.dot(frame.y));
        // A vertex on the axis (apex, pole, cap centre) has no direction and says nothing about phase.
        if u.hypot(v) > inside {
            angles.push(v.atan2(u).rem_euclid(std::f64::consts::TAU));
        }
    }
    if angles.len() < 3 {
        return None;
    }
    angles.sort_by(f64::total_cmp);
    let mut sides = 1u32;
    let mut last = angles[0];
    for &angle in &angles[1..] {
        if angle - last > APART {
            sides += 1;
            last = angle;
        }
    }
    // A ring closing across zero would count its first direction twice.
    if sides > 1 && std::f64::consts::TAU - angles[angles.len() - 1] + angles[0] <= APART {
        sides -= 1;
    }
    if !(3..=MOST_SIDES).contains(&sides) {
        return None;
    }
    // The phase as the circular mean of all directions modulo the step, robust to the wrap at zero.
    let step = std::f64::consts::TAU / f64::from(sides);
    let (mut sin, mut cos) = (0.0, 0.0);
    for &angle in &angles {
        let (s, c) = (angle / step * std::f64::consts::TAU).sin_cos();
        sin += s;
        cos += c;
    }
    Some((sides, sin.atan2(cos) / std::f64::consts::TAU * step))
}

/// The side count from which an extruded polygon is read as a cylinder rather than a prism.
///
/// The one unmeasurable judgement: a 16-sided prism and a 16-segment cylinder are identical
/// triangles. Either rebuilds the body exactly; only the name and the editable parameter differ.
const ROUND_SIDES: u32 = 16;

/// The most sides a recognised polygon may have, the registry's own prism limit.
const MOST_SIDES: u32 = 128;

/// The angle apart two ring vertices must be to count as different directions: a tenth of a
/// 128-gon's step.
const APART: f64 = std::f64::consts::TAU / 1280.0;

/// The least off-axis distance that still counts as a direction, for very small rings.
const ON_AXIS: f64 = 1e-9;

/// How many of the largest faces are searched for one square to the axis.
const FACINGS_SQUARE: usize = 8;

/// How nearly square to the axis a face must be (about 1.5 degrees) to set the second direction.
const SQUARE_ENOUGH: f64 = 0.026;
