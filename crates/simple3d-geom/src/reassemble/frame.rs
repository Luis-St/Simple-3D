//! Which way a body is turned: candidate directions, and the angles for one.

use super::*;
use crate::mesh::Mesh;

/// An orthonormal frame: the directions of a shape's own X, Y and Z.
#[derive(Clone, Copy, Debug)]
pub(super) struct Frame {
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
}

impl Frame {
    /// A frame with Z `z` and X from `hint` made square to it. A parallel or zero hint is replaced
    /// by the world axis least like `z`.
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

    /// The frame turned by `radians` about its Z: a round shape's tessellation phase.
    pub(super) fn turned(self, radians: f64) -> Frame {
        let (s, c) = radians.sin_cos();
        let x = self.x * c + self.y * s;
        Frame { x, y: self.z.cross(x), z: self.z }
    }

    /// A world point in this frame's coordinates.
    pub(super) fn local(&self, p: Vec3) -> Vec3 {
        Vec3::new(p.dot(self.x), p.dot(self.y), p.dot(self.z))
    }

    /// A local vector back in world coordinates.
    pub(super) fn world(&self, v: Vec3) -> Vec3 {
        self.x * v.x + self.y * v.y + self.z * v.z
    }

    /// The frame as a node's `rotation` angles in degrees.
    ///
    /// Nodes turn about X, then Y, then Z ([`Vec3::rotate_xyz_deg`]), so the matrix is
    /// `Rz * Ry * Rx`. In gimbal lock only the sum of the X and Z turns is recoverable; Z is set to
    /// zero, matching the transform panel's convention.
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

/// The same directions, each renamed after and turned towards its nearest world axis.
///
/// A box has 24 equally true descriptions; this picks the one a person would have typed, so an
/// unturned box comes back unturned with its width still the width.
pub(super) fn squared_to_world(frame: Frame) -> Frame {
    let mine = [frame.x, frame.y, frame.z];
    let world = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
    let mut chosen = [Vec3::ZERO; 3];
    let mut spent = [false; 3];
    let mut placed = [false; 3];
    // Best match first, so a clearly axis-aligned direction is not lost to an earlier near miss.
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
    // The third from the cross product, keeping the frame right-handed; a box is symmetric either way.
    Frame { x: chosen[0], y: chosen[1], z: chosen[0].cross(chosen[1]) }
}

/// One flat region: a facing direction and the area facing it.
#[derive(Clone, Copy)]
pub(super) struct Facing {
    pub normal: Vec3,
    pub area: f64,
}

/// The directions the surface faces, largest area first.
///
/// Gathered by direction, not adjacency, so a box gives six entries however tessellated. Capped,
/// since a sphere faces every direction.
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
                // Re-averaged by area as it grows, so unevenly tessellated faces keep their true direction.
                let facing = &mut found[at];
                facing.normal = (facing.normal * facing.area + normal * area).normalized();
                facing.area += area;
            }
            // Past the cap the body is round, and more facets are not worth the cost to every fit.
            None if found.len() < MOST_FACINGS => found.push(Facing { normal, area }),
            None => {}
        }
    }
    found.sort_by(|a, b| b.area.total_cmp(&a.area));
    found
}

/// The directions a body might be built along, best first:
///
/// * the line through the two busiest vertices: a sphere's pole axis;
/// * large flat faces' normals: a cylinder's cap, a box's side;
/// * where two large faces meet: a prism's axis seen from its sides;
/// * the world axes, for unrotated shapes.
pub(super) fn axes(mesh: &Mesh, facings: &[Facing]) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = Vec::new();
    let mut offer = |direction: Vec3| {
        let direction = upright(direction.normalized());
        if direction.length() < 0.5 || out.len() >= MOST_AXES {
            return;
        }
        // A direction and its opposite are the same axis; trying both would double the work.
        if out.iter().any(|kept: &Vec3| kept.dot(direction).abs() > SAME_WAY) {
            return;
        }
        out.push(direction);
    };
    // First, since a sphere has nothing else to be found by and its equal facets would otherwise
    // fill the list first.
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

/// Three angles with arithmetic noise removed, so an unturned shape does not store `-6.4e-15`
/// or show a non-zero rotation.
fn settled(angles: Vec3) -> Vec3 {
    let round = |a: f64| (a * ROUNDING).round() / ROUNDING;
    Vec3::new(round(angles.x), round(angles.y), round(angles.z))
}

/// How finely a fitted angle is kept: to a millionth of a degree.
const ROUNDING: f64 = 1e6;

/// The same axis, pointing up rather than down.
///
/// Opposite directions are the same axis to every fit but not the same rotation; otherwise an
/// upright pin could come back turned 180 degrees about X. Preference: Z, then Y, then X.
fn upright(v: Vec3) -> Vec3 {
    match [v.z, v.y, v.x].into_iter().find(|c| c.abs() > 1e-9) {
        Some(c) if c < 0.0 => -v,
        _ => v,
    }
}

/// The line through the two opposite vertices where the most triangles meet: a tessellated
/// sphere's pole axis. On other bodies it is just one more direction.
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
    // Exactly two, opposite: one is a cone's apex (already found), three is irregular.
    let [a, b] = poles[..] else { return None };
    (a.normalized().dot(b.normalized()) < -0.9).then(|| (a - b).normalized())
}

/// How nearly parallel two directions must be to count as one: about 1.5 degrees.
const SAME_WAY: f64 = 0.999_66;

/// The most facing directions gathered; past this the body is round.
const MOST_FACINGS: usize = 64;

/// How many of the largest faces are tried. Six is every face of a box.
const FACINGS_TRIED: usize = 6;

/// The most directions tried per body; each costs a fit of every shape.
const MOST_AXES: usize = 12;
