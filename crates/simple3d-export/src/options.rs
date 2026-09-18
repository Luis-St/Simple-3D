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
    /// Compress the parts of a 3MF package rather than storing them. On by
    /// default: a model part is XML and compresses by about a factor of four,
    /// and every program that reads 3MF reads a deflated one -- it is what they
    /// all write. Off is for the rare case of wanting to read the XML out of
    /// the package with something that cannot decompress it.
    ///
    /// Only 3MF is a package; the other formats are single files and ignore it.
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
