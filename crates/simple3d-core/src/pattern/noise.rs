//! A bit of randomness on top of the rule (issue 79).
//!
//! A pattern is exact by construction, which is right for bolt holes and wrong
//! for planks: a deck laid out on a perfect grid reads as wallpaper. This nudges,
//! turns and resizes each copy a little off where the rule put it, by no more
//! than the amounts asked for.
//!
//! The scatter is *derived*, not drawn: copy `i` of seed `s` always lands in the
//! same place, so a file laid out today is laid out identically when it is
//! opened next year, on another machine, in another build. Nothing is stored per
//! copy -- a handful of numbers describe the whole scatter -- and the seed is
//! what makes a scatter that happens to look wrong into one the user can simply
//! step past.

use super::*;
use crate::primitive::{Params, ParamsExt};
use crate::xform::Xform;
use simple3d_geom::Vec3;

/// Every parameter the scatter is made of, in the order its window shows them.
pub fn noise_keys() -> &'static [&'static str] {
    &["noise_x", "noise_y", "noise_z", "noise_turn", "noise_axis", "noise_scale", "noise_seed", "noise_keep_first"]
}

/// How far the copies may wander, as read off a pattern's parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Noise {
    /// The most a copy may be nudged along each axis, either way.
    pub offset: Vec3,
    /// The most a copy may be turned, either way, in degrees.
    pub turn: f64,
    /// What it is turned about: 0, 1 or 2 for one axis, 3 for all three --
    /// each by its own amount, up to `turn`.
    pub axis: usize,
    /// The most a copy may be made bigger or smaller, as a fraction: 0.1 is
    /// anything from a tenth smaller to a tenth bigger.
    pub scale: f64,
    pub seed: u32,
    /// Whether the original is left exactly where it is.
    pub keep_first: bool,
}

impl Noise {
    pub fn of(params: &Params) -> Noise {
        Noise {
            offset: Vec3::new(params.num("noise_x").abs(), params.num("noise_y").abs(), params.num("noise_z").abs()),
            turn: params.num("noise_turn").abs(),
            axis: params.int("noise_axis").min(3) as usize,
            scale: params.int("noise_scale").min(90) as f64 / 100.0,
            seed: params.int("noise_seed"),
            keep_first: params.get("noise_keep_first").is_some_and(|v| v.as_bool()),
        }
    }

    /// Whether any is asked for. A pattern with none pays nothing for this.
    pub fn wanted(&self) -> bool {
        self.offset.length() > 1e-9 || self.turn > 1e-9 || self.scale > 1e-9
    }

    /// Where copy `index` actually goes, in its own frame: resized and turned
    /// where it stands, then nudged.
    ///
    /// Each number has a channel of its own, and the four a scatter has always
    /// had keep theirs, so a file scattered before the turn could be about all
    /// three axes or the size could change lands every copy where it did.
    pub fn wobble(&self, index: usize) -> Xform {
        let offset = Vec3::new(
            self.offset.x * self.signed(index, 0),
            self.offset.y * self.signed(index, 1),
            self.offset.z * self.signed(index, 2),
        );
        let rotation = if self.axis >= 3 {
            Vec3::new(
                self.turn * self.signed(index, 3),
                self.turn * self.signed(index, 4),
                self.turn * self.signed(index, 5),
            )
        } else {
            rotation_about(self.axis, self.turn * self.signed(index, 3))
        };
        let size = 1.0 + self.scale * self.signed(index, 6);
        Xform::from_pos_rot_scale(offset, rotation, Vec3::splat(size))
    }

    /// One of this copy's numbers, in -1..1.
    fn signed(&self, index: usize, channel: u32) -> f64 {
        let bits = mix((self.seed as u64) << 40 ^ (index as u64) << 8 ^ channel as u64);
        // The top 53 bits are the ones a double can hold exactly.
        (bits >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

/// Nudge every copy off where the rule put it.
///
/// In the copy's own frame, composed on the *inside*: a plank is turned about
/// its own middle and shifted from where it stands, not swung round the centre
/// of the whole pattern.
pub(crate) fn scatter(params: &Params, copies: &mut [Instance]) {
    let noise = Noise::of(params);
    if !noise.wanted() {
        return;
    }
    let skip = usize::from(noise.keep_first);
    for (index, copy) in copies.iter_mut().enumerate().skip(skip) {
        copy.xform = copy.xform.compose(&noise.wobble(index));
    }
}

/// A scatter that can make neighbouring copies meet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crowding {
    /// The narrowest gap the rule leaves between two neighbours.
    pub gap: f64,
    /// How much of it two neighbours wandering towards each other can close.
    pub reach: f64,
}

/// Whether the scatter can close a gap the rule leaves, for copies of a shape
/// `size` across (issue 79).
///
/// Copies that meet are welded into one body -- a pattern unions its copies --
/// so a scatter of planks that is a millimetre too generous stops being planks.
/// This is an estimate rather than a collision test: it asks, for each stage
/// that lays copies side by side, how far apart two neighbours stand, how much
/// of that the shape itself fills, and whether the two of them wandering
/// towards each other at the most the scatter allows could cover the rest.
/// Neighbours that touch already are left out: that is the rule's doing, and
/// the scatter only answers for the gaps it can close.
pub fn crowding(params: &Params, size: Vec3) -> Option<Crowding> {
    let noise = Noise::of(params);
    if !noise.wanted() {
        return None;
    }
    let mut worst: Option<Crowding> = None;
    for stage in rule_stages(params) {
        if stage.mode == StageMode::Mirror || stage.copies() < 2 {
            continue;
        }
        let gaps = (stage.count - 2) as f64;
        let (dir, spacing) = match stage.mode {
            StageMode::Move => {
                let length = stage.step.length();
                if length < 1e-9 {
                    continue;
                }
                // A run whose gaps shrink is tightest at its far end, and one
                // whose gaps come round on a cycle at its narrowest gap.
                let narrowest = (0..stage.count - 1).map(|j| stage.gap_after(j)).fold(0.0, f64::min);
                (stage.step * (1.0 / length), length + narrowest)
            }
            _ => {
                let radius = stage.radius.min(stage.radius + stage.growth * gaps).abs();
                let chord = 2.0 * radius * (stage.turn.to_radians() / 2.0).sin().abs();
                // Neighbours round a turn stand side by side along its tangent.
                (unit(3 - stage.axis - radial_axis(stage.axis)), chord.hypot(stage.rise))
            }
        };
        let across = |v: Vec3| dir.x.abs() * v.x + dir.y.abs() * v.y + dir.z.abs() * v.z;
        let extent = across(size);
        let gap = spacing - extent;
        if gap <= 1e-9 {
            continue;
        }
        // Each of the two can come the whole jitter towards the other, grow by
        // its share of the size jitter, and swing a corner round by its turn.
        let turn = noise.turn.min(90.0).to_radians().sin();
        let reach = 2.0 * across(noise.offset) + extent * noise.scale + size.length() * turn;
        if reach > gap && worst.is_none_or(|w| gap < w.gap) {
            worst = Some(Crowding { gap, reach });
        }
    }
    worst
}

/// SplitMix64's finaliser: a cheap, well-mixed hash, so two copies whose
/// indices differ by one land nowhere near each other.
fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
