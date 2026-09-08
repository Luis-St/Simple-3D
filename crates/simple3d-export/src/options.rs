//! What an export was asked for, and how its progress is reported back.

use super::*;
#[derive(Clone, Debug)]
pub struct Options {
    pub format: Format,
    /// Uniform scale applied at export time, for producing scaled prints without
    /// touching the model. 1.0 leaves every dimension exactly as entered.
    pub scale: f64,
    /// Written into the file for formats that record it. Everything upstream is
    /// millimetres, so this is only ever anything else if a caller asks.
    pub unit: Unit3mf,
    /// Skip the manifold check. Only for a user who has read the warning and
    /// chosen to write the file anyway.
    pub allow_invalid: bool,
    /// What the objects of the export are (issue 58). Only a caller that passes
    /// more than one [`Part`] has anything to separate, and only a
    /// [`Format::keeps_objects_separate`] format can hold it.
    pub bodies: BodyMode,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: Format::ThreeMf,
            scale: 1.0,
            unit: Unit3mf::Millimeter,
            allow_invalid: false,
            bodies: BodyMode::One,
        }
    }
}

/// Progress reporting and cancellation. Return `false` to cancel.
pub type Progress<'a> = &'a mut dyn FnMut(f32) -> bool;
