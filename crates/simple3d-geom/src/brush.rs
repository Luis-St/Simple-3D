//! A brush over a mesh's surface (issue 127): what one dab does to the vertices under it, as a
//! terrain editor's brush does to a height field.
//!
//! A dab lands on the surface at a point and reaches the vertices within its radius, connected to
//! that point over the surface (found by the caller, who has the topology). Each is moved by the
//! mode, scaled by the strength, the falloff of its distance from the centre and its weight from
//! [`crate::shaping::weights`], which keeps a hole's rim and a print's base in place. Moving
//! vertices never adds or removes a triangle, so a closed mesh stays closed; [`refine_within`] adds
//! the vertices a coarse mesh needs before a dab can shape it.

mod dab;
pub use dab::{dab, Surface};
mod refine;
pub use refine::refine_within;
#[cfg(test)]
mod tests;

use crate::shaping::Shaping;
use serde::{Deserialize, Serialize};

/// The brush's settings, remembered between sessions.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Brush {
    pub mode: Mode,
    /// How far a dab reaches, in millimetres.
    pub radius: f64,
    /// How much one dab does, from 0 to 1: raising lifts the centre by a tenth of the radius at full
    /// strength, smoothing and flattening go the whole way there.
    pub strength: f64,
    pub falloff: Falloff,
    /// Whether a dab first splits the edges under it longer than `edge`, so a coarse mesh has the
    /// vertices to take the stroke.
    pub detail: bool,
    /// The longest edge adding detail leaves, in millimetres. Kept while `detail` is off.
    pub edge: f64,
    /// The noise mode's feature size, in millimetres, and which pattern.
    pub scale: f64,
    pub seed: u32,
    /// Whether a hole's rim stays where it is ([`Shaping::keep_boundaries`]).
    pub keep_boundaries: bool,
    /// Whether the bottom stays where it is, and how high ([`Shaping::keep_base`]).
    pub keep_base: bool,
    pub base: f64,
}

impl Default for Brush {
    fn default() -> Brush {
        Brush {
            mode: Mode::Raise,
            radius: 5.0,
            strength: 0.3,
            falloff: Falloff::Smooth,
            detail: false,
            edge: 0.5,
            scale: 2.0,
            seed: 1,
            keep_boundaries: true,
            keep_base: false,
            base: 1.0,
        }
    }
}

impl Brush {
    /// The smallest and largest radius, in millimetres, and the strength's range.
    pub const RADIUS: (f64, f64) = (0.05, 1_000.0);
    pub const STRENGTH: (f64, f64) = (0.01, 1.0);

    /// How far along the stroke one dab is from the next, in millimetres: a quarter of the radius,
    /// close enough that the dabs blend into one stroke.
    pub fn spacing(&self) -> f64 {
        self.radius * 0.25
    }

    /// The keep settings as the mesh functions read them, for [`crate::shaping::weights`].
    pub fn keeping(&self) -> Shaping {
        Shaping {
            keep_boundaries: self.keep_boundaries,
            keep_base: self.keep_base,
            base: self.base,
            ..Shaping::default()
        }
    }
}

/// What a dab does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Lift the surface along its mean normal under the brush.
    #[default]
    Raise,
    /// Press it in.
    Lower,
    /// Take out bumps, keeping the volume (Taubin's smoothing, as the mesh function does).
    Smooth,
    /// Pull the surface towards the plane fitted under the brush.
    Flatten,
    /// Push it in and out by a noise pattern ([`crate::shaping::Noise`]).
    Noise,
}

impl Mode {
    pub const ALL: [Mode; 5] = [Mode::Raise, Mode::Lower, Mode::Smooth, Mode::Flatten, Mode::Noise];

    pub fn label(self) -> &'static str {
        match self {
            Mode::Raise => "Raise",
            Mode::Lower => "Lower",
            Mode::Smooth => "Smooth",
            Mode::Flatten => "Flatten",
            Mode::Noise => "Noise",
        }
    }

    /// Raise for lower and the other way round; the rest stay as they are.
    pub fn inverted(self) -> Mode {
        match self {
            Mode::Raise => Mode::Lower,
            Mode::Lower => Mode::Raise,
            other => other,
        }
    }
}

/// How a dab's effect fades from its centre to its edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Falloff {
    /// A rounded hill, fading out gently at both ends.
    #[default]
    Smooth,
    /// A pointed peak, strongest right at the centre.
    Sharp,
    /// A plateau, full strength over most of the radius and fading only at the edge.
    Flat,
}

impl Falloff {
    pub const ALL: [Falloff; 3] = [Falloff::Smooth, Falloff::Sharp, Falloff::Flat];

    pub fn label(self) -> &'static str {
        match self {
            Falloff::Smooth => "Smooth",
            Falloff::Sharp => "Sharp",
            Falloff::Flat => "Flat",
        }
    }

    /// The share of the full effect at `distance` from the centre of a dab of `radius`: 1 at the
    /// centre, 0 at the edge and beyond.
    pub fn at(self, distance: f64, radius: f64) -> f64 {
        let t = (1.0 - distance / radius.max(1e-12)).clamp(0.0, 1.0);
        let smooth = |t: f64| t * t * (3.0 - 2.0 * t);
        match self {
            Falloff::Smooth => smooth(t),
            Falloff::Sharp => t * t,
            Falloff::Flat => smooth((t * 4.0).min(1.0)),
        }
    }
}
