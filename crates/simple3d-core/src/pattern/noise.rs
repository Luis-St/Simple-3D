//! A bit of randomness on top of the rule (issue 79).
//!
//! A pattern is exact by construction, which is right for bolt holes and wrong
//! for planks: a deck laid out on a perfect grid reads as wallpaper. This nudges
//! and turns each copy a little off where the rule put it, by no more than the
//! amounts asked for.
//!
//! The scatter is *derived*, not drawn: copy `i` of seed `s` always lands in the
//! same place, so a file laid out today is laid out identically when it is
//! opened next year, on another machine, in another build. Nothing is stored per
//! copy -- four numbers describe the whole scatter -- and the seed is what makes
//! a scatter that happens to look wrong into one the user can simply step past.

use super::*;
use crate::primitive::{Params, ParamsExt};
use crate::xform::Xform;
use simple3d_geom::Vec3;

/// Every parameter the scatter is made of.
pub fn noise_keys() -> &'static [&'static str] {
    &["noise_x", "noise_y", "noise_z", "noise_turn", "noise_axis", "noise_seed"]
}

/// How far the copies may wander, as read off a pattern's parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Noise {
    /// The most a copy may be nudged along each axis, either way.
    pub offset: Vec3,
    /// The most a copy may be turned about `axis`, either way, in degrees.
    pub turn: f64,
    pub axis: usize,
    pub seed: u32,
}

impl Noise {
    pub fn of(params: &Params) -> Noise {
        Noise {
            offset: Vec3::new(params.num("noise_x").abs(), params.num("noise_y").abs(), params.num("noise_z").abs()),
            turn: params.num("noise_turn").abs(),
            axis: params.int("noise_axis").min(2) as usize,
            seed: params.int("noise_seed"),
        }
    }

    /// Whether any is asked for. A pattern with none pays nothing for this.
    pub fn wanted(&self) -> bool {
        self.offset.length() > 1e-9 || self.turn > 1e-9
    }

    /// Where copy `index` actually goes, in its own frame: turned about the
    /// chosen axis where it stands, then nudged.
    pub fn wobble(&self, index: usize) -> Xform {
        let offset = Vec3::new(
            self.offset.x * self.signed(index, 0),
            self.offset.y * self.signed(index, 1),
            self.offset.z * self.signed(index, 2),
        );
        Xform::from_pos_rot(offset, rotation_about(self.axis, self.turn * self.signed(index, 3)))
    }

    /// One of this copy's four numbers, in -1..1.
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
    for (index, copy) in copies.iter_mut().enumerate() {
        copy.xform = copy.xform.compose(&noise.wobble(index));
    }
}

/// SplitMix64's finaliser: a cheap, well-mixed hash, so two copies whose
/// indices differ by one land nowhere near each other.
fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
