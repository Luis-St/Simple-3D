//! Fitting a shape to one body, and measuring whether it is that shape.
//!
//! Every fit here is direct rather than searched: given a direction to try, the
//! numbers of the shape follow from the body's own extents and from a circle
//! fitted to the ring of vertices at each end, in closed form. What is searched
//! is only the *direction* -- see [`super::frame::axes`] for the dozen worth
//! trying -- and every shape is fitted along every one of them.
//!
//! What decides between the fits is [`measure`], not the fits themselves: each
//! one is generated exactly as the registry would build it and the body's own
//! surface is measured against it. A fit that is nonsense measures as nonsense,
//! so a wrong guess costs a measurement rather than a wrong answer.

use super::frame::{self, Frame};
use super::*;
use crate::mesh::Mesh;
use crate::simplify::measure;

/// Work out what one body is. Never fails to produce a part: a body nothing
/// was recognised in keeps its triangles.
pub(super) fn part_of(body: &Mesh, plan: &Reassemble, give_up: Abandon<'_>) -> Option<Part> {
    let bounds = bounds_of(&body.positions)?;
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
/// Two passes, because the measurement is the expensive part and there are
/// sixty of them to make. The first is over a thinned-out sample of the
/// surface, which is enough to throw out a fit that is wrong by a millimetre
/// and cheap enough to make sixty times; the second is over every point of it,
/// made only for the fits that survived and in the order the shapes are
/// preferred in, and it is the one that decides.
fn recognise(body: &Mesh, tolerance: f64, give_up: Abandon<'_>) -> Option<Option<Fitted>> {
    let facings = frame::facings(body);
    let mut fits: Vec<Fitted> = Vec::new();
    for axis in frame::axes(body, &facings) {
        if give_up() {
            return None;
        }
        fits.extend(along(body, axis, &facings, tolerance));
    }
    // A dozen directions on a body with an axis mostly produce the *same* fit
    // over again -- every face of a box is found from all six of its normals,
    // and a box fit is put on the world's axes before it is returned, so the
    // twelve come back identical to the last bit. Measuring one of them is
    // measuring all of them.
    same_again(&mut fits);
    let samples = samples(body);
    let stride = samples.len().div_ceil(SCREENED).max(1);
    let whole = stride == 1;
    let screen: Vec<Vec3> = samples.iter().copied().step_by(stride).collect();
    for fit in fits.iter_mut() {
        fit.deviation = deviation(&screen, fit);
    }
    // The shape first, the closeness second: a cube is a box and a four-sided
    // prism at once, both to the last bit, and which of the two it is called
    // must not come down to which of two zeroes is the smaller.
    fits.retain(|fit| fit.deviation <= tolerance * SCREEN_SLACK);
    fits.sort_by(|a, b| rank(a.shape).cmp(&rank(b.shape)).then(a.deviation.total_cmp(&b.deviation)));
    // The best few of each kind rather than the best few outright. A dozen
    // directions produce a dozen fits, and for a body with an axis they are
    // mostly the *same* fit found again: a cylinder is a box to within a
    // fraction of a millimetre from every direction at once, so ten box fits
    // that will each fail would otherwise fill the list and the one cylinder
    // fit -- exact, and sorted behind them because a box is preferred -- would
    // never be measured at all.
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
        // Measured again only where the screen was a thinning of the surface.
        // On a body small enough to have been screened over every one of its
        // points, the screen *was* the measurement.
        if !whole {
            fit.deviation = deviation(&samples, &fit);
        }
        if fit.deviation <= tolerance {
            return Some(Some(fit));
        }
    }
    Some(None)
}

/// Drop the fits that are another fit over again: the same shape, standing in
/// the same place, turned the same way.
///
/// Their parameters are not compared, and do not need to be: a fit's numbers
/// follow from the body and the frame, so two fits of one kind that agree on
/// where the shape stands and how it is turned agree on everything.
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

/// How far the body's surface is from a fit, in millimetres: the furthest any
/// of the sampled points is from the shape as it would be built.
fn deviation(samples: &[Vec3], fit: &Fitted) -> f64 {
    measure::furthest_from(samples, &fit.shape.mesh().transformed(fit.centre, fit.frame.rotation_deg()))
}

/// The points of the body a fit is measured at: its corners, and the middle of
/// every triangle.
///
/// The middles are what makes the measurement mean anything. Every corner of a
/// plate with a hole bored through it sits on the surface of the plate's own
/// bounding box, so a measurement taken at corners alone calls the plate a
/// solid box and loses the hole; the triangles lining the hole are in the
/// middle of that box, and a measurement that asks about them is not fooled.
fn samples(body: &Mesh) -> Vec<Vec3> {
    let mut out = body.positions.clone();
    out.extend(body.indices.iter().map(|tri| {
        (body.positions[tri[0] as usize] + body.positions[tri[1] as usize] + body.positions[tri[2] as usize]) / 3.0
    }));
    out
}

/// Which shape is preferred where two of them fit equally well. Lower is
/// sooner.
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

/// The body as a box standing along `axis`: the smallest box that holds it in
/// the frame the axis and the largest face square to it make.
fn as_box(body: &Mesh, axis: Vec3, facings: &[frame::Facing]) -> Option<Fitted> {
    // A box is found by its faces, so the frame's second direction is a face
    // too -- the largest one square to the axis. Anything else would be a box
    // turned about its own axis by an arbitrary amount, which holds the body
    // but is not the box it is.
    let square = facings
        .iter()
        .take(FACINGS_SQUARE)
        .find(|facing| facing.normal.dot(axis).abs() < SQUARE_ENOUGH)
        .map_or(Vec3::ZERO, |facing| facing.normal);
    let frame = frame::squared_to_world(Frame::new(axis, square));
    let local: Vec<Vec3> = body.positions.iter().map(|&p| frame.local(p)).collect();
    let (lo, hi) = bounds_of(&local)?;
    let size = hi - lo;
    (size.x > 0.0 && size.y > 0.0 && size.z > 0.0).then_some(Fitted {
        shape: Shape::Box { width: size.x, depth: size.y, height: size.z },
        frame,
        centre: frame.world((lo + hi) * 0.5),
        deviation: f64::MAX,
    })
}

/// The body as something extruded along `axis` from a regular polygon: a
/// cylinder, a prism or a cone.
///
/// All three have the same mesh -- a ring of vertices at each end and nothing
/// in between -- so all three are found the same way and told apart afterwards
/// by the two radii and by how many sides there are.
fn as_round(body: &Mesh, axis: Vec3, tolerance: f64) -> Option<Fitted> {
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
            // A vertex between the two ends: whatever this body is, it is not
            // one polygon extruded into another. The cheapest and by far the
            // most common way out of this fit, and what keeps a sphere from
            // being run through the whole of it.
            return None;
        }
    }
    let plain = Frame::new(axis, Vec3::ZERO);
    let (bottom_centre, bottom_radius) = ring(&bottom, &plain)?;
    let (top_centre, top_radius) = ring(&top, &plain)?;
    let (wider, wider_radius) = if bottom.len() >= top.len() { (&bottom, bottom_radius) } else { (&top, top_radius) };
    let (sides, phase) = spokes(wider, &plain, (bottom_centre + top_centre) * 0.5, wider_radius)?;
    let frame = plain.turned(phase);
    // The axis runs through the middle of the two rings; the shape's own centre
    // is halfway up it.
    let perpendicular = (bottom_centre + top_centre) * 0.5;
    let centre = perpendicular - axis * perpendicular.dot(axis) + axis * ((low + high) / 2.0);
    let shape = if (bottom_radius - top_radius).abs() > tolerance {
        Shape::Cone { bottom_diameter: bottom_radius * 2.0, top_diameter: top_radius * 2.0, height, segments: sides }
    } else {
        // The two radii agree to within the tolerance, so their sum is the
        // diameter and using both rings rather than one halves what a vertex
        // out of place can do to it.
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
fn as_sphere(body: &Mesh, axis: Vec3) -> Option<Fitted> {
    let count = body.positions.len();
    if count < 8 {
        return None;
    }
    let centre = body.positions.iter().fold(Vec3::ZERO, |sum, &p| sum + p) / count as f64;
    let plain = Frame::new(axis, Vec3::ZERO);
    // How far the widest of it stands off the axis, which is what tells a pole
    // from a vertex that is merely near one.
    let radius =
        body.positions.iter().map(|&p| (p - centre).dot(plain.x).hypot((p - centre).dot(plain.y))).fold(0.0, f64::max);
    let (sides, phase) = spokes(&body.positions, &plain, centre, radius)?;
    // A tessellated ellipsoid of this many meridians has exactly this many
    // vertices -- a ring of them at each latitude but the two poles, which are
    // one vertex each. A body with any other number is not one, and saying so
    // here rather than by measuring is what keeps the fit cheap: a cylinder of
    // sixty-four segments reads as sixty-four meridians, and the sphere that
    // would be built to measure it against has four thousand triangles against
    // the body's two hundred and fifty.
    let bands = (sides / 2).max(2) as usize;
    if count != sides as usize * (bands - 1) + 2 {
        return None;
    }
    let frame = plain.turned(phase);
    let local: Vec<Vec3> = body.positions.iter().map(|&p| frame.local(p - centre)).collect();
    let (lo, hi) = bounds_of(&local)?;
    let size = hi - lo;
    (size.x > 0.0 && size.y > 0.0 && size.z > 0.0).then_some(Fitted {
        shape: Shape::Sphere { diameter_x: size.x, diameter_y: size.y, diameter_z: size.z, segments: sides },
        frame,
        centre,
        deviation: f64::MAX,
    })
}

/// The circle a ring of vertices sits on, square to the frame's axis: a point
/// on the axis of that circle, and its radius.
///
/// Fitted by least squares rather than taken from the extents, because the
/// extents of a polygon are not the polygon: the widest points of a triangle
/// across two perpendicular directions give a centre that is not its centre and
/// a radius that is not its radius, and a three-sided prism is a shape somebody
/// really does model.
///
/// Fitted twice, and the second time without the points sitting in the middle
/// of it. A cap of more than four corners is fanned from a vertex added at its
/// centre -- see the extruder's own `flat_cap` -- and that one vertex is not on
/// the circle the other thirty-two are: left in, it pulls the fitted radius in
/// by a percent or so, which is a good deal more than the tolerance and was
/// enough to stop a plain cylinder being recognised as one.
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

/// The circle through a set of points, by the algebraic least-squares fit.
///
/// Fewer than three points cannot describe a circle and are taken as an end
/// that has closed to a point, which is what a cone's apex is.
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
        // Every point on one line: a ring of two vertices, or a fit along an
        // axis this body has no ring around at all.
        return None;
    }
    let (d, e) = ((-suz * svv + svz * suv) / det, (-svz * suu + suz * suv) / det);
    let f = -sz / count as f64;
    let (cu, cv) = (-d / 2.0, -e / 2.0);
    let radius = (cu * cu + cv * cv - f).max(0.0).sqrt();
    Some((middle + frame.x * cu + frame.y * cv, radius))
}

/// How many evenly spaced directions a ring of vertices stands in around
/// `centre`, and how far the first of them is turned from the frame's own X.
///
/// The count is the shape's segments and the turn is its phase, and both have
/// to be right for the fit to be worth measuring: a cylinder of twelve segments
/// generated a sixth of a turn out of step with the body is out by the whole
/// sagitta of its facets everywhere, which is not a near miss but a different
/// solid.
fn spokes(points: &[Vec3], frame: &Frame, centre: Vec3, radius: f64) -> Option<(u32, f64)> {
    // What counts as standing on the axis, measured against how far out the
    // ring itself is rather than as a distance.
    //
    // An absolute figure cannot work, and the way it failed is worth keeping
    // in mind: a round cap is fanned from a vertex added at its centre, which
    // in the numbers a generator produces is *exactly* on the axis -- and is
    // not, once the mesh has been through a project file, where a stored
    // position is an `f32`. A hundredth of a micron off the axis still points
    // somewhere, and that direction was counted as one more side: a hexagonal
    // prism came back seven-sided and then failed to be anything at all.
    let inside = (radius / 2.0).max(ON_AXIS);
    let mut angles: Vec<f64> = Vec::new();
    for &p in points {
        let from = p - centre;
        let (u, v) = (from.dot(frame.x), from.dot(frame.y));
        // A vertex on the axis itself -- a cone's apex, a sphere's pole, the
        // vertex a round cap is fanned from -- has no direction round it and
        // says nothing about the phase.
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
    // The first and the last are the same direction when the ring closes across
    // zero, and counting both would make a hexagon seven-sided.
    if sides > 1 && std::f64::consts::TAU - angles[angles.len() - 1] + angles[0] <= APART {
        sides -= 1;
    }
    if !(3..=MOST_SIDES).contains(&sides) {
        return None;
    }
    // The phase, as the circular mean of every direction taken modulo the step
    // between them: an average that cannot be spoiled by the wrap at zero, and
    // one that uses the whole ring rather than whichever vertex happens to come
    // first.
    let step = std::f64::consts::TAU / f64::from(sides);
    let (mut sin, mut cos) = (0.0, 0.0);
    for &angle in &angles {
        let (s, c) = (angle / step * std::f64::consts::TAU).sin_cos();
        sin += s;
        cos += c;
    }
    Some((sides, sin.atan2(cos) / std::f64::consts::TAU * step))
}

/// The fewest triangles a body can have and still be a shape: a tetrahedron,
/// which is the smallest closed surface there is.
const FEWEST_TRIANGLES: usize = 4;

/// How near two fits have to stand, in millimetres and in the length of the
/// difference between their directions, to be the same fit found twice.
const SAME_FIT: f64 = 1e-9;

/// How many points a fit is screened over before it is measured in full.
const SCREENED: usize = 1_500;

/// How much further than the tolerance a screened fit may measure and still be
/// worth measuring in full.
///
/// A thinned sample can miss the one place a shape is worst by, so the screen
/// is deliberately loose: it is there to throw out the fits that are wrong by
/// the width of the part, not to decide anything.
const SCREEN_SLACK: f64 = 8.0;

/// How many of the fits that survived the screen are measured in full. Past a
/// handful they are all wrong in the same way, and the body is a mesh.
const CONFIRMED: usize = 12;

/// How many fits of any one shape are measured in full.
const PER_KIND: usize = 2;

/// How many shapes there are to be preferred between -- see [`rank`].
const KINDS: usize = 6;

/// How many sides a polygon is taken to have before the shape it is extruded
/// from is read as a circle rather than as a polygon somebody meant.
///
/// The one judgement in the whole of the recognition that cannot be measured:
/// the registry builds a regular prism and a cylinder from the same extruded
/// polygon, so a sixteen-sided prism and a cylinder of sixteen segments are the
/// same triangles and nothing in them says which was meant. Sixteen is where
/// the answer changes because it is where the *reason* changes: a prism is a
/// shape with sides, and by sixteen of them nobody is counting -- while the
/// coarsest tessellation anything here would call round is about that. Either
/// answer rebuilds the body exactly; only its name and which number is turned
/// to change it differ.
const ROUND_SIDES: u32 = 16;

/// The most sides a polygon may have to be recognised at all -- the registry's
/// own limit on a prism's sides, and past any tessellation a printer is given.
const MOST_SIDES: u32 = 128;

/// How far apart two vertices have to stand around a ring, in radians, to be
/// standing in different directions. A tenth of the step of a 128-sided
/// polygon, which is the finest ring that is recognised at all.
const APART: f64 = std::f64::consts::TAU / 1280.0;

/// The least a vertex may stand off the axis and still point anywhere, for a
/// ring so small that half its radius is smaller still.
const ON_AXIS: f64 = 1e-9;

/// How many of the largest faces are looked through for one square to the axis.
const FACINGS_SQUARE: usize = 8;

/// How nearly square to the axis a face has to be to set the frame's second
/// direction: within about a degree and a half of it.
const SQUARE_ENOUGH: f64 = 0.026;
