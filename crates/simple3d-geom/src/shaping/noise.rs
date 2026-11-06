//! Seeded 3D noise: Perlin's gradient noise, and Worley's cells.

use crate::vec3::Vec3;

/// The noise for one seed: a shuffled permutation table, so a seed always gives the same pattern
/// and neighbouring seeds unrelated ones.
pub struct Noise {
    perm: [u8; 512],
    seed: u32,
}

impl Noise {
    pub fn new(seed: u32) -> Noise {
        let mut table: [u8; 256] = std::array::from_fn(|i| i as u8);
        let mut state = mix(u64::from(seed) ^ 0x9E37_79B9_7F4A_7C15);
        // Fisher-Yates with a xorshift: enough randomness for a pattern, and the same on every machine.
        for i in (1..256).rev() {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            table.swap(i, (state % (i as u64 + 1)) as usize);
        }
        Noise { perm: std::array::from_fn(|i| table[i & 255]), seed }
    }

    fn hash(&self, x: i64, y: i64, z: i64) -> u8 {
        let p = |v: i64| self.perm[(v & 255) as usize] as i64;
        self.perm[((p(p(x) + y) + z) & 255) as usize]
    }

    /// Improved Perlin noise at `p`, in about -1 to 1, zero at every lattice point.
    pub fn perlin(&self, p: Vec3) -> f64 {
        let (fx, fy, fz) = (p.x.floor(), p.y.floor(), p.z.floor());
        let (x, y, z) = (fx as i64, fy as i64, fz as i64);
        let (dx, dy, dz) = (p.x - fx, p.y - fy, p.z - fz);
        let (u, v, w) = (fade(dx), fade(dy), fade(dz));
        let corner = |i: i64, j: i64, k: i64| {
            gradient(self.hash(x + i, y + j, z + k), dx - i as f64, dy - j as f64, dz - k as f64)
        };
        let lerp = |t: f64, a: f64, b: f64| a + t * (b - a);
        let x00 = lerp(u, corner(0, 0, 0), corner(1, 0, 0));
        let x10 = lerp(u, corner(0, 1, 0), corner(1, 1, 0));
        let x01 = lerp(u, corner(0, 0, 1), corner(1, 0, 1));
        let x11 = lerp(u, corner(0, 1, 1), corner(1, 1, 1));
        lerp(w, lerp(v, x00, x10), lerp(v, x01, x11))
    }

    /// Raised cells around one random point per unit cell, 0 to 1: 0 along the creases where two
    /// cells meet, rising in a rounded dome towards each cell's middle.
    ///
    /// From the gap between the nearest and second-nearest point (Worley's F2 - F1), which is zero
    /// exactly on the line between two cells, so the creases stay sharp; the distance to the nearest
    /// point alone gives soft cones that blur into noise once layered.
    pub fn cells(&self, p: Vec3) -> f64 {
        let (x, y, z) = (p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64);
        let (mut nearest, mut second) = (f64::INFINITY, f64::INFINITY);
        for i in -1..=1 {
            for j in -1..=1 {
                for k in -1..=1 {
                    let (cx, cy, cz) = (x + i, y + j, z + k);
                    let h = mix((cx as u64).wrapping_mul(0x8DA6_B343)
                        ^ (cy as u64).wrapping_mul(0xD816_3841)
                        ^ (cz as u64).wrapping_mul(0xCB1A_B31F)
                        ^ u64::from(self.seed).wrapping_mul(0x9E37_79B9));
                    let unit = |shift: u32| ((h >> shift) & 0xFFFF) as f64 / 65_535.0;
                    let point = Vec3::new(cx as f64 + unit(0), cy as f64 + unit(16), cz as f64 + unit(32));
                    let d = (point - p).length();
                    if d < nearest {
                        (nearest, second) = (d, nearest);
                    } else if d < second {
                        second = d;
                    }
                }
            }
        }
        // Half a cell's gap is the middle of a typical cell; the dome is flat-topped from there.
        let rise = 1.0 - ((second - nearest) * 2.0).min(1.0);
        1.0 - rise * rise
    }

    /// `layers` octaves of `one` summed, each at twice the frequency and half the weight of the one
    /// before and shifted so their lattices do not line up, scaled back to the range of one.
    pub fn layered(&self, p: Vec3, layers: u32, one: impl Fn(&Noise, Vec3) -> f64) -> f64 {
        let (mut sum, mut weight, mut total, mut frequency) = (0.0, 1.0, 0.0, 1.0);
        for layer in 0..layers.max(1) {
            let shift = Vec3::splat(f64::from(layer) * 17.31);
            sum += weight * one(self, p * frequency + shift);
            total += weight;
            weight *= 0.5;
            frequency *= 2.0;
        }
        sum / total
    }
}

/// Perlin's quintic easing, so the noise is smooth across cell faces.
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// One of Perlin's twelve edge gradients, picked by `hash`, dotted with the offset.
fn gradient(hash: u8, x: f64, y: f64, z: f64) -> f64 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
}

/// SplitMix64's finaliser: a well-spread 64-bit hash of `x`.
fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}
