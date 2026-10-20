//! What a toothed wheel is built around its outline: a bore with a keyway, a hub, and teeth turning
//! along a helix up its thickness.

use crate::mesh::Mesh;
use crate::revolve::grid_mesh;
use crate::vec3::Vec3;
use std::f64::consts::{PI, TAU};

/// The wheel round a toothed outline; [`Body::mesh`] builds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub thickness: f64,
    pub bore: f64,
    /// A collar on top: its diameter and height; none while either is 0 or it is no wider than the bore.
    pub hub: (f64, f64),
    /// A keyway in the bore: its width and how far it reaches past the bore; none while either is 0.
    pub key: (f64, f64),
    /// How far the teeth turn per unit of height, in radians: 0 for straight teeth.
    pub twist: f64,
    /// Whether the twist turns back halfway up, so the side forces cancel.
    pub herringbone: bool,
}

impl Body {
    /// A straight wheel of `thickness` round a bore, with no hub or keyway.
    pub fn new(thickness: f64, bore: f64) -> Body {
        Body { thickness, bore, hub: (0.0, 0.0), key: (0.0, 0.0), twist: 0.0, herringbone: false }
    }

    /// The bore's radius, narrowed to leave a rim under teeth whose roots are `root` out.
    pub fn bore_radius(&self, root: f64) -> f64 {
        self.bore.clamp(0.0, root * 2.0 * 0.9) / 2.0
    }

    /// The hub's radius and height as built: none (0, 0) when it would not stand proud of the bore. It
    /// is held inside the root circle, the top face left between it and the teeth.
    pub fn hub_built(&self, root: f64) -> (f64, f64) {
        let (diameter, height) = self.hub;
        let radius = (diameter / 2.0).min(root * 0.98);
        if height <= 0.0 || radius <= self.bore_radius(root) + 1e-6 {
            (0.0, 0.0)
        } else {
            (radius, height)
        }
    }

    /// The keyway as built: half its width and the radius its far side reaches, or none.
    fn key_built(&self, root: f64) -> Option<(f64, f64)> {
        let hole = self.bore_radius(root);
        let (width, depth) = self.key;
        let half = (width / 2.0).min(hole * 0.95);
        let reach = (hole + depth).min(root * 0.95);
        (hole > 1e-6 && half > 1e-6 && reach > hole + 1e-6).then_some((half, reach))
    }

    /// The whole height, the hub's included.
    pub fn height(&self, root: f64) -> f64 {
        self.thickness + self.hub_built(root).1
    }

    /// How far the teeth have turned `t` up from the bottom face.
    pub(super) fn turn_at(&self, t: f64) -> f64 {
        if self.herringbone {
            self.twist * t.min(self.thickness - t)
        } else {
            self.twist * t
        }
    }

    /// The outline turned and lifted to every level, a level every couple of degrees of turn; a
    /// herringbone's turn peaks halfway, on a level.
    pub(super) fn levels(&self, outline: &[(f64, f64)], bottom: f64) -> Vec<Vec<Vec3>> {
        let most = if self.herringbone { self.twist * self.thickness / 2.0 } else { self.twist * self.thickness };
        let levels = match self.twist == 0.0 {
            true => 1,
            false if self.herringbone => ((most.abs() / 0.03).ceil() as usize).max(1) * 2,
            false => ((most.abs() / 0.03).ceil() as usize).max(2),
        };
        (0..=levels)
            .map(|k| {
                let t = self.thickness * k as f64 / levels as f64;
                turned(outline, self.turn_at(t), bottom + t)
            })
            .collect()
    }

    /// The wheel round `outline`, a star-shaped outline turning steadily about the origin whose teeth
    /// root `root` out, centred on its whole height.
    pub fn mesh(&self, mut outline: Vec<(f64, f64)>, root: f64) -> Mesh {
        let bore = self.bore_radius(root);
        let key = self.key_built(root);
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
        let (hub, hub_height) = self.hub_built(root);
        let bottom = -self.height(root) / 2.0;
        let top = bottom + self.thickness;
        let mut rings = self.levels(&outline, bottom);
        let mut summit = top;
        if hub_height > 0.0 {
            // The hub ring follows the top's teeth round, as the face between them is paired point by point.
            let collar: Vec<(f64, f64)> = directions.iter().map(|&a| (hub * a.cos(), hub * a.sin())).collect();
            let turn = self.turn_at(self.thickness);
            rings.push(turned(&collar, turn, top));
            summit = top + hub_height;
            rings.push(turned(&collar, turn, summit));
        }
        rings.push(turned(&inner, 0.0, summit));
        rings.push(turned(&inner, 0.0, bottom));
        grid_mesh(&rings, true)
    }
}

/// `points` turned by `turn` about the Z axis and set at height `z`.
pub(super) fn turned(points: &[(f64, f64)], turn: f64, z: f64) -> Vec<Vec3> {
    let (s, c) = turn.sin_cos();
    points.iter().map(|&(x, y)| Vec3::new(x * c - y * s, x * s + y * c, z)).collect()
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
pub(super) fn insert_directions(outline: &mut Vec<(f64, f64)>, directions: &[f64]) {
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
