//! A bit of randomness on top of the rule (issue 79).
//!
//! Nudges, turns and resizes each copy within the requested amounts. Derived, not stored: copy
//! `i` of seed `s` always lands in the same place on any machine or build, and a new seed gives a
//! new scatter.

use super::*;
use crate::primitive::{ParamValue, Params, ParamsExt};
use crate::xform::Xform;
use simple3d_geom::Vec3;

/// Every scatter parameter, in the order its window shows them.
pub fn noise_keys() -> &'static [&'static str] {
    &[
        "noise_x",
        "noise_y",
        "noise_z",
        "noise_turn_x",
        "noise_turn_y",
        "noise_turn_z",
        "noise_scale",
        "noise_seed",
        "noise_keep_first",
    ]
}

/// The turn about each axis, by axis.
pub const NOISE_TURN_KEYS: [&str; 3] = ["noise_turn_x", "noise_turn_y", "noise_turn_z"];

/// Update an older scatter's single turn with an axis choice into per-axis amounts (issue 79).
/// Reads `stored` and writes `out`; a scatter already using per-axis turns is left alone.
pub fn migrate_noise(stored: &Params, out: &mut Params) {
    if NOISE_TURN_KEYS.iter().any(|key| stored.contains_key(*key)) {
        return;
    }
    let Some(turn) = stored.get("noise_turn").map(|v| v.as_f64()) else { return };
    let axis = stored.get("noise_axis").map_or(2, |v| v.as_u32()).min(3) as usize;
    for (index, key) in NOISE_TURN_KEYS.iter().enumerate() {
        let amount = if axis == 3 || axis == index { turn } else { 0.0 };
        out.insert((*key).to_string(), ParamValue::Angle(amount.rem_euclid(360.0)));
    }
}

/// How far the copies may wander, read from a pattern's parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Noise {
    /// The most a copy may be nudged along each axis, either way.
    pub offset: Vec3,
    /// The most a copy may be turned about each axis, either way, in degrees.
    pub turn: Vec3,
    /// The most a copy may scale either way, as a fraction: 0.1 is plus or minus a tenth.
    pub scale: f64,
    pub seed: u32,
    /// Whether the original is left exactly where it is.
    pub keep_first: bool,
}

impl Noise {
    pub fn of(params: &Params) -> Noise {
        Noise {
            offset: Vec3::new(params.num("noise_x").abs(), params.num("noise_y").abs(), params.num("noise_z").abs()),
            turn: Vec3::new(
                params.num(NOISE_TURN_KEYS[0]).abs(),
                params.num(NOISE_TURN_KEYS[1]).abs(),
                params.num(NOISE_TURN_KEYS[2]).abs(),
            ),
            scale: params.int("noise_scale").min(90) as f64 / 100.0,
            seed: params.int("noise_seed"),
            keep_first: params.get("noise_keep_first").is_some_and(|v| v.as_bool()),
        }
    }

    /// Whether any is asked for; a pattern without pays nothing.
    pub fn wanted(&self) -> bool {
        self.offset.length() > 1e-9 || self.turn.length() > 1e-9 || self.scale > 1e-9
    }

    /// Where copy `index` goes in its own frame: resized and turned in place, then nudged.
    ///
    /// Each number has its own channel, and the original four keep theirs, so older files land every
    /// copy where they did. A single-axis turn uses the old turn channel.
    pub fn wobble(&self, index: usize) -> Xform {
        let offset = Vec3::new(
            self.offset.x * self.signed(index, 0),
            self.offset.y * self.signed(index, 1),
            self.offset.z * self.signed(index, 2),
        );
        let turns = [self.turn.x, self.turn.y, self.turn.z];
        let about: Vec<usize> = (0..3).filter(|axis| turns[*axis] > 1e-9).collect();
        let rotation = match about.as_slice() {
            [axis] => rotation_about(*axis, turns[*axis] * self.signed(index, 3)),
            _ => Vec3::new(
                self.turn.x * self.signed(index, 3),
                self.turn.y * self.signed(index, 4),
                self.turn.z * self.signed(index, 5),
            ),
        };
        let size = 1.0 + self.scale * self.signed(index, 6);
        Xform::from_pos_rot_scale(offset, rotation, Vec3::splat(size))
    }

    /// One of this copy's numbers, in -1..1.
    fn signed(&self, index: usize, channel: u32) -> f64 {
        let bits = mix((self.seed as u64) << 40 ^ (index as u64) << 8 ^ channel as u64);
        // The top 53 bits are the ones a double holds exactly.
        (bits >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }
}

/// Nudge every copy, composed on the inside: turned about its own middle, not the pattern's centre.
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

/// Whether the scatter can close a gap between copies of a shape `size` across (issue 79), which
/// would weld them. An estimate per side-by-side stage, not a collision test; neighbours already
/// touching are the rule's doing and ignored.
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
                // A shrinking or cycling run is tightest at its narrowest gap.
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
        // Each neighbour can come the full jitter closer, grow by its share, and swing a corner round.
        let turn = noise.turn.x.max(noise.turn.y).max(noise.turn.z).min(90.0).to_radians().sin();
        let reach = 2.0 * across(noise.offset) + extent * noise.scale + size.length() * turn;
        if reach > gap && worst.is_none_or(|w| gap < w.gap) {
            worst = Some(Crowding { gap, reach });
        }
    }
    worst
}

/// SplitMix64's finaliser, so neighbouring indices land far apart.
fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
