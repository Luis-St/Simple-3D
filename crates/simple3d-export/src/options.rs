//! What an export was asked for, and how its progress is reported back.

use super::*;
#[derive(Clone, Debug)]
pub struct Options {
    pub format: Format,
    /// Uniform export scale, for scaled prints without touching the model; 1.0 changes nothing.
    pub scale: f64,
    /// The unit written into formats that record one; everything upstream is millimetres.
    pub unit: Unit3mf,
    /// Skip the manifold check, only after the user accepts the warning.
    pub allow_invalid: bool,
    /// What the export's objects are (issue 58); only meaningful with several [`Part`]s and a
    /// [`Format::keeps_objects_separate`] format.
    pub bodies: BodyMode,
    /// Compress a 3MF's parts (about 4x for model XML); on by default, since every reader handles it.
    /// Ignored by single-file formats.
    pub compress: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: Format::ThreeMf,
            scale: 1.0,
            unit: Unit3mf::Millimeter,
            allow_invalid: false,
            bodies: BodyMode::One,
            compress: true,
        }
    }
}

/// Progress reporting and cancellation. Return `false` to cancel.
pub type Progress<'a> = &'a mut dyn FnMut(f32) -> bool;
