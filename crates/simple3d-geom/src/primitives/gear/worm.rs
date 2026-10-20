//! A worm (issue 123): a screw whose thread is a rack's teeth wound round it, turning a helical gear a
//! tooth for every start each turn. It gives a large reduction in one step and holds against the load.

use super::super::thread::{helix_mesh, ThreadProfile};
use crate::mesh::Mesh;
use std::f64::consts::PI;

/// A worm of `module` (axial pitch over pi) with `starts` threads, `pitch_diameter` across its pitch
/// line and `length` long, centred on the origin along Z. Its thread is the involute rack's: 20 degree
/// flanks, a module above the pitch line and a quarter more below. The gear it drives is a helical one
/// of the same module with its helix at the worm's lead angle, atan(module * starts / pitch diameter).
pub fn worm_mesh(module: f64, starts: u32, pitch_diameter: f64, length: f64, left_hand: bool, segments: u32) -> Mesh {
    let m = module.max(1e-6);
    let pitch = PI * m;
    let slope = 20f64.to_radians().tan();
    // Half the pitch is tooth at the pitch line, which lies a module below the crest.
    let crest = 0.5 - 2.0 * m * slope / pitch;
    let flank = 2.25 * m * slope / pitch;
    let profile = ThreadProfile { depth: 2.25 * m, crest, flank, root: 1.0 - crest - 2.0 * flank };
    helix_mesh(pitch_diameter + 2.0 * m, pitch, starts, length, profile, left_hand, segments)
}
