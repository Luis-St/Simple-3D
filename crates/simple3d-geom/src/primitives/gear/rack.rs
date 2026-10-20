//! A gear rack (issue 123): a straight bar of teeth that meshes with a spur gear of the same module and
//! profile, turning its rotation into a straight push.

use super::ToothProfile;
use crate::mesh::Mesh;
use crate::vec3::Vec3;
use std::f64::consts::PI;

/// Heights a tooth's flank is drawn at.
const FLANK_POINTS: usize = 8;

/// Everything a rack is made of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RackSpec {
    pub teeth: u32,
    pub module: f64,
    pub profile: ToothProfile,
    /// Along Z, the gear's thickness.
    pub thickness: f64,
    /// Across Y, from the back to the tips.
    pub height: f64,
    /// Helix angle in degrees, 0 for straight teeth.
    pub helix: f64,
    pub herringbone: bool,
}

impl RackSpec {
    /// How far the teeth reach above the pitch line: a module, or a circular arc's half circle.
    pub fn addendum(&self) -> f64 {
        match self.profile {
            ToothProfile::CircularArc => PI * self.module / 4.0,
            _ => self.module,
        }
    }

    /// The height as built: at least a module of back under the teeth.
    pub fn height_built(&self) -> f64 {
        self.height.max(self.addendum() + 2.25 * self.module)
    }

    /// Half a tooth's width `h` above the pitch line, a quarter pitch there.
    fn half_width(&self, h: f64) -> f64 {
        let m = self.module;
        let quarter = PI * m / 4.0;
        let width = match self.profile {
            ToothProfile::Involute => quarter - h * 20f64.to_radians().tan(),
            ToothProfile::Cycloidal => {
                // A cycloid rolled by the same circle as the gear's tips, two modules across, on the
                // side of the pitch line it is traced on.
                let rolling = 2.0 * m;
                let t = (1.0 - h.abs() / rolling).clamp(-1.0, 1.0).acos();
                quarter - h.signum() * rolling * (t - t.sin())
            }
            ToothProfile::CircularArc if h > 0.0 => (quarter * quarter - h * h).max(0.0).sqrt(),
            ToothProfile::CircularArc => quarter,
        };
        // Short of the next tooth at the root.
        width.clamp(0.0, PI * m / 2.0 * 0.95)
    }

    /// The top of the rack from left to right in its own section, Y up, from the back at Y = 0.
    fn crests(&self) -> Vec<(f64, f64)> {
        let m = self.module;
        let pitch = PI * m;
        let length = pitch * self.teeth.max(1) as f64;
        let line = self.height_built() - self.addendum();
        let (root, top) = (-1.25 * m, self.addendum());
        // Closer together near the tip, where a circular arc turns fastest.
        let heights: Vec<f64> = (0..=FLANK_POINTS)
            .map(|i| root + (top - root) * (PI / 2.0 * i as f64 / FLANK_POINTS as f64).sin())
            .collect();
        let mut out = vec![(-length / 2.0, line + root)];
        for k in 0..self.teeth.max(1) {
            let middle = -length / 2.0 + pitch * (k as f64 + 0.5);
            let flank: Vec<(f64, f64)> = heights.iter().map(|&h| (self.half_width(h), line + h)).collect();
            out.extend(flank.iter().map(|&(w, y)| (middle - w, y)));
            // A pointed tooth has one tip, not two at the same place.
            let skip = usize::from(flank[FLANK_POINTS].0 < 1e-9);
            out.extend(flank.iter().rev().skip(skip).map(|&(w, y)| (middle + w, y)));
        }
        out.push((length / 2.0, line + root));
        out.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12);
        out
    }
}

/// The rack `spec` describes, centred on its bounds: its length along X, its teeth pointing to +Y and
/// its thickness along Z, the teeth slanting along X with height when helical.
pub fn rack_mesh(spec: &RackSpec) -> Mesh {
    let crests = spec.crests();
    let back = -spec.height_built() / 2.0;
    // The section: the crests, then the back under every place they change direction.
    let mut columns: Vec<f64> = crests.iter().map(|p| p.0).collect();
    columns.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    let mut section: Vec<(f64, f64)> = crests.iter().map(|&(x, y)| (x, y + back)).collect();
    section.extend(columns.iter().rev().map(|&x| (x, back)));
    let ends = cap(&section[..crests.len()], &columns, back);
    // The slant along X with height, centred so the rack stays centred along X.
    let slope = spec.helix.to_radians().tan();
    let half = spec.thickness / 2.0;
    let shift_at = |t: f64| if spec.herringbone { slope * t.min(spec.thickness - t) } else { slope * t };
    let heights = match (slope == 0.0, spec.herringbone) {
        (false, true) => vec![0.0, half, spec.thickness],
        _ => vec![0.0, spec.thickness],
    };
    let most = if spec.herringbone { shift_at(half) } else { shift_at(spec.thickness) };
    let place = |(x, y): (f64, f64), t: f64| Vec3::new(x + shift_at(t) - most / 2.0, y, t - half);
    let rings: Vec<Vec<Vec3>> = heights.iter().map(|&t| section.iter().map(|&p| place(p, t)).collect()).collect();
    let n = section.len();
    let mut mesh = Mesh::new();
    for pair in rings.windows(2) {
        for i in 0..n {
            let j = (i + 1) % n;
            mesh.push_triangle(pair[0][i], pair[0][j], pair[1][j]);
            mesh.push_triangle(pair[0][i], pair[1][j], pair[1][i]);
        }
    }
    for [a, b, c] in ends {
        mesh.push_triangle(place(a, 0.0), place(b, 0.0), place(c, 0.0));
        mesh.push_triangle(place(a, spec.thickness), place(c, spec.thickness), place(b, spec.thickness));
    }
    mesh.turned_outward()
}

/// The rack's end face as triangles, counter-clockwise, `crests` being its top from left to right and
/// `columns` every X the crests change direction at. Each strip from the back up to the crests is
/// fanned from its middle, so points along its upright sides are kept and nothing is left with no width.
fn cap(crests: &[(f64, f64)], columns: &[f64], back: f64) -> Vec<[(f64, f64); 3]> {
    let same = |a: f64, b: f64| (a - b).abs() < 1e-12;
    // The crest points at `x` no higher than `below`, upwards.
    let side = |x: f64, below: f64| {
        let mut points: Vec<(f64, f64)> =
            crests.iter().copied().filter(|p| same(p.0, x) && p.1 <= below + 1e-12).collect();
        points.sort_by(|a, b| a.1.total_cmp(&b.1));
        points
    };
    let mut out = Vec::new();
    for strip in columns.windows(2) {
        let (left, right) = (strip[0], strip[1]);
        // The crest edge across the strip, from the last point at its left to the first at its right.
        let Some(i) = (0..crests.len() - 1).find(|&i| same(crests[i].0, left) && same(crests[i + 1].0, right)) else {
            continue;
        };
        let mut boundary = vec![(left, back), (right, back)];
        boundary.extend(side(right, crests[i + 1].1));
        boundary.extend(side(left, crests[i].1).into_iter().rev());
        let count = boundary.len() as f64;
        let centre = boundary.iter().fold((0.0, 0.0), |sum, p| (sum.0 + p.0 / count, sum.1 + p.1 / count));
        for k in 0..boundary.len() {
            out.push([centre, boundary[k], boundary[(k + 1) % boundary.len()]]);
        }
    }
    out
}
