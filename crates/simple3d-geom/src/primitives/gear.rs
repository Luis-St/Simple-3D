//! A spur gear (issue 123) of involute, cycloidal or circular-arc teeth, straight, helical or herringbone,
//! with a hub and a keyway.

mod tooth;
pub use tooth::ToothProfile;

use crate::mesh::Mesh;
use crate::revolve::grid_mesh;
use crate::vec3::Vec3;
use std::f64::consts::{PI, TAU};

/// Everything a gear is made of; [`GearSpec::new`] gives a standard straight gear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GearSpec {
    pub teeth: u32,
    /// Pitch diameter over teeth.
    pub module: f64,
    /// The involute's pressure angle in degrees.
    pub pressure_angle: f64,
    pub profile: ToothProfile,
    pub thickness: f64,
    pub bore: f64,
    /// Helix angle in degrees, 0 for straight teeth.
    pub helix: f64,
    /// Whether the helix turns back halfway up, so the side forces cancel.
    pub herringbone: bool,
    /// A collar on top: its diameter and height; none while either is 0 or it is no wider than the bore.
    pub hub: (f64, f64),
    /// A keyway in the bore: its width and how far it reaches past the bore; none while either is 0.
    pub key: (f64, f64),
}

impl GearSpec {
    pub fn new(teeth: u32, module: f64, pressure_angle: f64, thickness: f64, bore: f64) -> GearSpec {
        GearSpec {
            teeth,
            module,
            pressure_angle,
            profile: ToothProfile::Involute,
            thickness,
            bore,
            helix: 0.0,
            herringbone: false,
            hub: (0.0, 0.0),
            key: (0.0, 0.0),
        }
    }

    pub fn tip_radius(&self) -> f64 {
        let pitch = self.pitch_radius();
        (pitch + self.profile.addendum(pitch, self.module, self.teeth.max(3) as f64)).max(1e-6)
    }

    fn pitch_radius(&self) -> f64 {
        self.module * self.teeth.max(3) as f64 / 2.0
    }

    /// The root circle, a quarter module deeper than the teeth reach, held inside the tip circle and off
    /// the middle.
    pub fn root_radius(&self) -> f64 {
        let tip = self.tip_radius();
        (self.pitch_radius() - self.module * 1.25).clamp(tip * 0.05, tip * 0.999)
    }

    /// The bore's radius, narrowed to leave a rim under the teeth.
    pub fn bore_radius(&self) -> f64 {
        self.bore.clamp(0.0, self.root_radius() * 2.0 * 0.9) / 2.0
    }

    /// The hub's radius and height as built: none (0, 0) when it would not stand proud of the bore. It
    /// is held inside the root circle, the top face left between it and the teeth.
    pub fn hub_built(&self) -> (f64, f64) {
        let (diameter, height) = self.hub;
        let radius = (diameter / 2.0).min(self.root_radius() * 0.98);
        if height <= 0.0 || radius <= self.bore_radius() + 1e-6 {
            (0.0, 0.0)
        } else {
            (radius, height)
        }
    }

    /// The keyway as built: half its width and the radius its far side reaches, or none.
    fn key_built(&self) -> Option<(f64, f64)> {
        let hole = self.bore_radius();
        let (width, depth) = self.key;
        let half = (width / 2.0).min(hole * 0.95);
        let reach = (hole + depth).min(self.root_radius() * 0.95);
        (hole > 1e-6 && half > 1e-6 && reach > hole + 1e-6).then_some((half, reach))
    }

    /// The whole height, the hub's included.
    pub fn height(&self) -> f64 {
        self.thickness + self.hub_built().1
    }
}

/// A spur gear of `teeth` teeth of `module` (pitch diameter over teeth) with the given pressure
/// angle, `thickness` tall, with a round bore; a tooth points along +X. A bore of 0 leaves it
/// solid; one reaching the teeth is narrowed to leave a rim.
pub fn spur_gear_mesh(teeth: u32, module: f64, pressure_angle: f64, thickness: f64, bore: f64) -> Mesh {
    gear_mesh(&GearSpec::new(teeth, module, pressure_angle, thickness, bore))
}

/// The gear `spec` describes, centred on its whole height with a tooth along +X at the bottom face.
pub fn gear_mesh(spec: &GearSpec) -> Mesh {
    let mut outline = gear_outline(spec);
    let bore = spec.bore_radius();
    let key = spec.key_built();
    if let Some((half, reach)) = key {
        // The keyway's corners, so its sides come out straight.
        let side = (bore * bore - half * half).max(0.0).sqrt();
        insert_directions(
            &mut outline,
            &[half.atan2(side), half.atan2(reach), (-half).atan2(reach), (-half).atan2(side)],
        );
    }
    let directions: Vec<f64> = outline.iter().map(|&(x, y)| y.atan2(x)).collect();
    let inner: Vec<(f64, f64)> = directions
        .iter()
        .map(|&a| {
            let r = bore_radius_at(a, bore, key);
            (r * a.cos(), r * a.sin())
        })
        .collect();
    let (hub, hub_height) = spec.hub_built();
    let bottom = -spec.height() / 2.0;
    let top = bottom + spec.thickness;
    // The teeth turn with height along the helix: by its tangent over the pitch radius, per height.
    let rate = spec.helix.to_radians().tan() / spec.pitch_radius().max(1e-9);
    let turn_at = |t: f64| if spec.herringbone { rate * t.min(spec.thickness - t) } else { rate * t };
    // A level every couple of degrees of turn; a herringbone's turn peaks halfway, on a level.
    let most = if spec.herringbone { rate * spec.thickness / 2.0 } else { rate * spec.thickness };
    let levels = match rate == 0.0 {
        true => 1,
        false if spec.herringbone => ((most.abs() / 0.03).ceil() as usize).max(1) * 2,
        false => ((most.abs() / 0.03).ceil() as usize).max(2),
    };
    let at = |points: &[(f64, f64)], turn: f64, z: f64| -> Vec<Vec3> {
        let (s, c) = turn.sin_cos();
        points.iter().map(|&(x, y)| Vec3::new(x * c - y * s, x * s + y * c, z)).collect()
    };
    let mut rings: Vec<Vec<Vec3>> = (0..=levels)
        .map(|k| {
            let t = spec.thickness * k as f64 / levels as f64;
            at(&outline, turn_at(t), bottom + t)
        })
        .collect();
    let mut summit = top;
    if hub_height > 0.0 {
        // The hub ring follows the top's teeth round, as the face between them is paired point by point.
        let collar: Vec<(f64, f64)> = directions.iter().map(|&a| (hub * a.cos(), hub * a.sin())).collect();
        let turn = turn_at(spec.thickness);
        rings.push(at(&collar, turn, top));
        summit = top + hub_height;
        rings.push(at(&collar, turn, summit));
    }
    rings.push(at(&inner, 0.0, summit));
    rings.push(at(&inner, 0.0, bottom));
    grid_mesh(&rings, true)
}

/// The bore's radius in direction `angle`: its circle, or the keyway's far side or walls along +X.
fn bore_radius_at(angle: f64, bore: f64, key: Option<(f64, f64)>) -> f64 {
    let Some((half, reach)) = key else { return bore };
    let (c, s) = (angle.cos(), angle.sin());
    if c <= 0.0 {
        return bore;
    }
    let to_end = reach / c;
    let slot = if (to_end * s).abs() <= half + 1e-12 { to_end } else { half / s.abs() };
    slot.max(bore)
}

/// Add points to a star-shaped outline turning steadily about the origin where the given directions
/// cross it, so another outline paired with it can have a corner there.
fn insert_directions(outline: &mut Vec<(f64, f64)>, directions: &[f64]) {
    for &want in directions {
        let n = outline.len();
        let angle = |p: (f64, f64)| p.1.atan2(p.0).rem_euclid(TAU);
        let want = want.rem_euclid(TAU);
        let Some(i) = (0..n).find(|&i| {
            let (a, b) = (angle(outline[i]), angle(outline[(i + 1) % n]));
            // A tooth's radial root edge has both ends at one angle, which rounding can make a whole turn.
            let span = (b - a).rem_euclid(TAU);
            span < PI && (want - a).rem_euclid(TAU) < span
        }) else {
            continue;
        };
        let (p, q) = (outline[i], outline[(i + 1) % n]);
        if (angle(p) - want).abs() < 1e-12 {
            continue;
        }
        // Where the ray meets the edge p-q.
        let (dx, dy) = (want.cos(), want.sin());
        let (ex, ey) = (q.0 - p.0, q.1 - p.1);
        let denominator = dx * ey - dy * ex;
        if denominator.abs() < 1e-15 {
            continue;
        }
        let t = (p.0 * ey - p.1 * ex) / denominator;
        outline.insert(i + 1, (t * dx, t * dy));
    }
}

/// The gear's outline, counter-clockwise from the tooth on +X, every point further round than the last.
fn gear_outline(spec: &GearSpec) -> Vec<(f64, f64)> {
    let teeth = spec.teeth.max(3);
    let z = teeth as f64;
    let pressure = spec.pressure_angle.clamp(0.1, 89.0).to_radians();
    let tip = spec.tip_radius();
    let root = spec.root_radius();
    // Kept off the middle, where the flanks of a small gear with a steep angle would cross, and short
    // of the gap's middle, where a widened tooth would meet the next.
    let flank: Vec<(f64, f64)> = tooth::flank(spec.profile, z, spec.pitch_radius(), pressure, root, tip)
        .into_iter()
        .map(|(r, off)| (r, off.clamp(PI / (20.0 * z), PI / z * 0.95)))
        .collect();
    // Below its lowest point the flank drops radially to the root.
    let (start, foot) = flank[0];
    let polar = |r: f64, angle: f64| (r * angle.cos(), r * angle.sin());
    let mut out = Vec::new();
    for tooth in 0..teeth {
        let middle = TAU * tooth as f64 / z;
        // The tip's middle first, so the tooth on +X has its point on the axis.
        out.push(polar(tip, middle));
        for &(r, off) in flank.iter().rev() {
            out.push(polar(r, middle + off));
        }
        if root < start {
            out.push(polar(root, middle + foot));
        }
        // The gap's middle, then the next tooth's leading flank up to its tip.
        let next = middle + TAU / z;
        out.push(polar(root, (middle + next) / 2.0));
        if root < start {
            out.push(polar(root, next - foot));
        }
        for &(r, off) in &flank {
            out.push(polar(r, next - off));
        }
    }
    out
}
