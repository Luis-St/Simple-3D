//! The tooth profiles a gear is cut with, each a flank of its own shape.

use std::f64::consts::PI;

/// Points along each flank of a tooth.
const FLANK_POINTS: usize = 6;

/// The curve a tooth's flanks follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToothProfile {
    /// The flanks unwind from the base circle: the everyday gear, meshing at any centre distance.
    Involute,
    /// Rounded tips rolled out by the circle that gives an eight-leaf pinion radial flanks, with radial
    /// flanks of its own below the pitch circle: the clock and watch gear.
    Cycloidal,
    /// A tooth ending in a half circle on the pitch circle, as in Wildhaber-Novikov gearing.
    CircularArc,
}

impl ToothProfile {
    /// From the registry's choice index, in declaration order.
    pub fn from_index(index: u32) -> ToothProfile {
        match index {
            1 => ToothProfile::Cycloidal,
            2 => ToothProfile::CircularArc,
            _ => ToothProfile::Involute,
        }
    }

    /// How far the teeth reach past the pitch circle of radius `pitch` on a gear of `teeth`.
    pub(super) fn addendum(self, pitch: f64, module: f64, teeth: f64) -> f64 {
        match self {
            // The arc's radius is half the tooth at the pitch circle.
            ToothProfile::CircularArc => pitch * (PI / (2.0 * teeth)).tan(),
            _ => module,
        }
    }
}

/// A tooth's trailing flank as radii and angles off the tooth's middle, from where it leaves the
/// root circle (or the circle it drops radially to the root from) out to the tip.
pub(super) fn flank(
    profile: ToothProfile,
    teeth: f64,
    pitch: f64,
    pressure: f64,
    root: f64,
    tip: f64,
) -> Vec<(f64, f64)> {
    // Half a tooth at the pitch circle, as an angle: a quarter pitch.
    let half = PI / (2.0 * teeth);
    let steps = |n: usize| (0..=n).map(move |i| i as f64 / n as f64);
    match profile {
        ToothProfile::Involute => {
            let base = pitch * pressure.cos();
            let involute = |angle: f64| angle.tan() - angle;
            // Less the involute's turn from the pitch circle; inside the base circle it drops radially.
            let start = root.max(base).min(tip);
            steps(FLANK_POINTS)
                .map(|f| {
                    let r = start + (tip - start) * f;
                    let at = (base / r.max(base)).clamp(-1.0, 1.0).acos();
                    (r, half + involute(pressure) - involute(at))
                })
                .collect()
        }
        ToothProfile::Cycloidal => {
            // An epicycloid off the pitch circle, rolled by half an eight-leaf pinion's pitch circle, as
            // that pinion's flanks are traced by; below it the flank is radial, traced by a circle half this
            // gear's own pitch circle rolled inside.
            let rolling = 2.0 * pitch / teeth;
            let k = (pitch + rolling) / rolling;
            let point = |t: f64| {
                let x = (pitch + rolling) * t.cos() - rolling * (k * t).cos();
                let y = (pitch + rolling) * t.sin() - rolling * (k * t).sin();
                (x.hypot(y), y.atan2(x))
            };
            // The roll that reaches the tip, found by halving; the curve climbs until its cusp at t = pi / k.
            let (mut low, mut high) = (0.0, PI / k);
            for _ in 0..60 {
                let middle = (low + high) / 2.0;
                if point(middle).0 < tip {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            // Twice the points, as the curve bends hardest near the tip.
            steps(2 * FLANK_POINTS)
                .map(|f| {
                    let (r, turned) = point(high * f);
                    (r, half - turned)
                })
                .collect()
        }
        ToothProfile::CircularArc => {
            // A circle on the pitch circle through the tooth's corners there, short of its top, which
            // the tip's middle point closes.
            let radius = tip - pitch;
            steps(FLANK_POINTS)
                .take(FLANK_POINTS)
                .map(|f| {
                    let (s, c) = (f * PI / 2.0).sin_cos();
                    let (x, y) = (pitch + radius * s, radius * c);
                    (x.hypot(y), y.atan2(x))
                })
                .collect()
        }
    }
}
