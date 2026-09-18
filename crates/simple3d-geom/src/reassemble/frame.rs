//! Which way a body is turned: the directions worth trying, and the angles
//! that stand for one of them.

use super::*;
use crate::mesh::Mesh;

/// An orthonormal frame, as the three directions a shape's own X, Y and Z point
/// in.
#[derive(Clone, Copy, Debug)]
pub(super) struct Frame {
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
}

impl Frame {
    /// A frame whose Z is `z` and whose X is `hint` brought square to it. A
    /// hint parallel to `z`, or no hint at all, is replaced by whichever world
    /// axis is least like `z`, so the frame is always well conditioned.
    pub(super) fn new(z: Vec3, hint: Vec3) -> Frame {
        let z = z.normalized();
        let square = hint - z * hint.dot(z);
        let x = if square.length() > 1e-6 {
            square.normalized()
        } else {
            let away = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)]
                .into_iter()
                .min_by(|a, b| a.dot(z).abs().total_cmp(&b.dot(z).abs()))
                .expect("three axes");
            (away - z * away.dot(z)).normalized()
        };
        Frame { x, y: z.cross(x), z }
    }

    /// The same frame turned by `radians` about its own Z: how the phase of a
    /// round shape's tessellation is written down.
    pub(super) fn turned(self, radians: f64) -> Frame {
        let (s, c) = radians.sin_cos();
        let x = self.x * c + self.y * s;
        Frame { x, y: self.z.cross(x), z: self.z }
    }

    /// A point of the world, in this frame's coordinates.
    pub(super) fn local(&self, p: Vec3) -> Vec3 {
        Vec3::new(p.dot(self.x), p.dot(self.y), p.dot(self.z))
    }

    /// The way back out.
    pub(super) fn world(&self, v: Vec3) -> Vec3 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }

    /// The frame as the three angles a node's `rotation` is, in degrees.
    ///
    /// The application turns a node about X, then Y, then Z (see
    /// [`Vec3::rotate_xyz_deg`]), so the matrix whose columns are these three
    /// directions is `Rz * Ry * Rx` and the angles are read back out of it in
    /// that order. Straight down the Y axis the X and Z turns are the same turn
    /// and only their sum can be recovered; the Z one is given up, which is the
    /// convention the transform panel already shows.
    pub(super) fn rotation_deg(&self) -> Vec3 {
        let (sin_y, cos_y) = (-self.x.z, (self.x.x * self.x.x + self.x.y * self.x.y).sqrt());
        if cos_y < 1e-9 {
            let x = (-self.z.y).atan2(self.y.y);
            return settled(Vec3::new(x.to_degrees(), sin_y.clamp(-1.0, 1.0).asin().to_degrees(), 0.0));
        }
        settled(Vec3::new(
            self.y.z.atan2(self.z.z).to_degrees(),
            sin_y.atan2(cos_y).to_degrees(),
            self.x.y.atan2(self.x.x).to_degrees(),
        ))
    }
}

/// The same three directions, each renamed after the world axis it is nearest
/// to and turned to face the same way as it.
///
/// For a box, whose three directions are found as three faces and could be
/// handed back in any order and either way round. Every one of the
/// twenty-four ways of describing a box is as true as the others, and this
/// picks the one somebody would have typed: an unturned box comes back
/// unturned, with the width it was made with still called the width, rather
/// than as the same solid stood on its side.
pub(super) fn squared_to_world(frame: Frame) -> Frame {
    let mine = [frame.x, frame.y, frame.z];
    let world = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
    let mut chosen = [Vec3::ZERO; 3];
    let mut spent = [false; 3];
    let mut placed = [false; 3];
    // Best match first, so the direction that is most plainly one of the
    // world's own gets it rather than losing it to a near miss taken earlier.
    for _ in 0..3 {
        let mut best = (0usize, 0usize, -1.0);
        for (i, direction) in mine.iter().enumerate() {
            for (j, axis) in world.iter().enumerate() {
                let alignment = direction.dot(*axis).abs();
                if !spent[i] && !placed[j] && alignment > best.2 {
                    best = (i, j, alignment);
                }
            }
        }
        let (i, j, _) = best;
        spent[i] = true;
        placed[j] = true;
        chosen[j] = if mine[i].dot(world[j]) < 0.0 { -mine[i] } else { mine[i] };
    }
    // The third from the other two rather than as it was found, so the frame is
    // right-handed however the faces happened to be turned. It is the same
    // direction or its opposite, and a box is symmetrical about either.
    Frame { x: chosen[0], y: chosen[1], z: chosen[0].cross(chosen[1]) }
}

/// One flat region of a surface: a direction its triangles face, and how much
/// area faces that way.
#[derive(Clone, Copy)]
pub(super) struct Facing {
    pub normal: Vec3,
    pub area: f64,
}

/// The directions the surface faces, largest area first.
///
/// Triangles are gathered by direction rather than by adjacency, so the six
/// faces of a box come back as six entries however finely each of them is
/// tessellated and wherever on the box they sit -- which is what makes them
/// worth asking about. The list is capped: a sphere faces every direction
/// there is, and running a thousand of them through every fit would cost more
/// than the whole of the rest of the work.
pub(super) fn facings(mesh: &Mesh) -> Vec<Facing> {
    let mut found: Vec<Facing> = Vec::new();
    for tri in &mesh.indices {
        let (a, b, c) =
            (mesh.positions[tri[0] as usize], mesh.positions[tri[1] as usize], mesh.positions[tri[2] as usize]);
        let cross = (b - a).cross(c - a);
        let area = cross.length() / 2.0;
        if area <= 0.0 {
            continue;
        }
        let normal = cross.normalized();
        match found.iter().position(|facing| facing.normal.dot(normal) > SAME_WAY) {
            Some(at) => {
                // Re-averaged as it grows, weighted by area, so a face that is
                // tessellated unevenly still comes back facing the way its
                // surface faces rather than the way its first triangle did.
                let facing = &mut found[at];
                facing.normal = (facing.normal * facing.area + normal * area).normalized();
                facing.area += area;
            }
            // Past the cap the body is round rather than flat-sided, and one
            // more facet of it is not worth the pass it would cost every fit.
            None if found.len() < MOST_FACINGS => found.push(Facing { normal, area }),
            None => {}
        }
    }
    found.sort_by(|a, b| b.area.total_cmp(&a.area));
    found
}

/// The directions a body might be built along, best first.
///
/// Four kinds of guess, and between them they cover everything the fits can
/// recognise:
///
/// * the line through the two vertices where the most triangles meet, which is
///   the pole axis of a sphere and nothing else -- a sphere faces every
///   direction equally and has no flat face to be found by;
/// * the way a large flat face looks -- the cap of a cylinder, a side of a box;
/// * the line where two large flat faces meet, which is the axis of a prism
///   seen by its sides rather than by its ends;
/// * the world's own axes, which is where an unrotated shape is.
pub(super) fn axes(mesh: &Mesh, facings: &[Facing]) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = Vec::new();
    let mut offer = |direction: Vec3| {
        let direction = upright(direction.normalized());
        if direction.length() < 0.5 || out.len() >= MOST_AXES {
            return;
        }
        // A direction and its opposite are the same axis: a cylinder stood on
        // its head is the same cylinder, and trying both would double the work
        // for no second answer.
        if out.iter().any(|kept: &Vec3| kept.dot(direction).abs() > SAME_WAY) {
            return;
        }
        out.push(direction);
    };
    // First, because it is the one guess that is not a guess: a body with two
    // vertices like that has them for a reason, and a sphere has nothing else
    // to be found by -- every facet of it is the same size, so the list would
    // otherwise fill with six of them before the poles were ever reached.
    if let Some(poles) = pole_axis(mesh) {
        offer(poles);
    }
    for facing in facings.iter().take(FACINGS_TRIED) {
        offer(facing.normal);
    }
    for (i, a) in facings.iter().take(FACINGS_TRIED).enumerate() {
        for b in facings.iter().take(FACINGS_TRIED).skip(i + 1) {
            offer(a.normal.cross(b.normal));
        }
    }
    offer(Vec3::new(0.0, 0.0, 1.0));
    offer(Vec3::new(1.0, 0.0, 0.0));
    offer(Vec3::new(0.0, 1.0, 0.0));
    out
}

/// Three angles with the arithmetic's own noise taken off them.
///
/// A fitted angle is an average over a ring of vertices, so a shape that is
/// plainly not turned at all comes out turned by a millionth of a millionth of
/// a degree. Over a metre that is a nanometre, which is below anything the
/// geometry means -- and what it costs to keep is a project file that says
/// `-6.4e-15` where a person would have typed nothing, and a properties panel
/// whose Rotation reads 0 while the node is not at 0.
fn settled(angles: Vec3) -> Vec3 {
    let round = |a: f64| (a * ROUNDING).round() / ROUNDING;
    Vec3::new(round(angles.x), round(angles.y), round(angles.z))
}

/// How finely a fitted angle is kept: to a millionth of a degree.
const ROUNDING: f64 = 1e6;

/// The same axis, pointing up rather than down.
///
/// A direction and its opposite are the same axis to every fit here -- a
/// cylinder stood on its head is the same cylinder -- but they are not the same
/// *rotation*, and the one that gets written on the node is the one somebody
/// reads. Taking the cap that happens to be found first left an upright pin
/// described as turned a hundred and eighty degrees about X, which is true and
/// is not what anybody would have typed.
///
/// Up, and then along Y, and then along X: Z first because that is the axis a
/// part stands on.
fn upright(v: Vec3) -> Vec3 {
    match [v.z, v.y, v.x].into_iter().find(|c| c.abs() > 1e-9) {
        Some(c) if c < 0.0 => -v,
        _ => v,
    }
}

/// The line through the two vertices where the most triangles meet, for a body
/// that has two such vertices facing each other across its middle.
///
/// That is the pole axis of a tessellated sphere, whose poles are a fan of one
/// triangle per meridian while every other vertex has six. Nothing else in the
/// list of shapes is found this way, and on a body that is not a sphere the
/// answer is simply one more direction that fits nothing.
fn pole_axis(mesh: &Mesh) -> Option<Vec3> {
    if mesh.positions.len() < 8 {
        return None;
    }
    let mut meeting = vec![0u32; mesh.positions.len()];
    for tri in &mesh.indices {
        for &v in tri {
            meeting[v as usize] += 1;
        }
    }
    let busiest = meeting.iter().copied().max()?;
    if busiest < 8 {
        return None;
    }
    let centre = bounds_of(&mesh.positions).map(|(lo, hi)| (lo + hi) * 0.5)?;
    let poles: Vec<Vec3> =
        (0..mesh.positions.len()).filter(|&v| meeting[v] == busiest).map(|v| mesh.positions[v] - centre).collect();
    // Two of them, opposite each other: one busy vertex is a cone's apex, which
    // the flat faces already found, and three is a tessellation with nothing
    // regular about it.
    let [a, b] = poles[..] else { return None };
    (a.normalized().dot(b.normalized()) < -0.9).then(|| (a - b).normalized())
}

/// How nearly parallel two directions have to be to count as one. A degree and
/// a half: a curve tessellated finely enough to face the same way twice within
/// that is a curve whose facets are not features.
const SAME_WAY: f64 = 0.999_66;

/// The most directions the surface is gathered into. Past this the body is
/// round rather than flat-sided, and what the fits need from it is its pole
/// axis rather than its thousandth facet.
const MOST_FACINGS: usize = 64;

/// How many of the largest faces are asked about. Six is every face of a box.
const FACINGS_TRIED: usize = 6;

/// The most directions a body is tried along. Each one costs a fit of every
/// shape, and the guesses are offered best first.
const MOST_AXES: usize = 12;
