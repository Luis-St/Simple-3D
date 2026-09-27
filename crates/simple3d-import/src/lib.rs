//! Mesh import (issue 105): 3MF, STL, OBJ and PLY, binary or text.
//!
//! Returns triangles and their names only; imports become stored mesh bodies (issue 80). File
//! structure (3MF objects, OBJ groups, STL solids) is kept as separate parts.
//!
//! * **Say what is wrong**: refusals name the reason and position.
//! * **Nothing half-read**: a parse yields the whole model or an error.
//! * **Cancellable with progress**, with the exporter's callback shape.

mod error;
pub(crate) use error::malformed;
pub use error::ImportError;
mod format;
pub use format::{Format, Unit};
/// The DEFLATE decoder, public so the export crate's encoder is tested against an independent
/// decoder.
pub mod inflate;
mod obj;
mod ply;
mod stl;
#[cfg(test)]
mod tests;
mod three_mf;
/// The package reader, public so the export crate's tests read packages back as a slicer would.
pub mod unzip;
mod xml;

use simple3d_geom::Mesh;

/// Progress reporting and cancellation, as for the exporter: return `false` to cancel.
pub type Progress<'a> = &'a mut dyn FnMut(f32) -> bool;

/// One object of a file: its triangles and name. The name is empty where the format has none
/// (STL), and the caller names the node after the file.
#[derive(Clone, Debug)]
pub struct Part {
    pub name: String,
    pub mesh: Mesh,
}

/// What a file held.
#[derive(Clone, Debug)]
pub struct Model {
    pub format: Format,
    /// The unit the file stated, if any; `parts` are already converted to millimetres.
    pub unit: Option<Unit>,
    pub parts: Vec<Part>,
}

impl Model {
    pub fn triangle_count(&self) -> usize {
        self.parts.iter().map(|part| part.mesh.triangle_count()).sum()
    }

    /// Everything in one mesh, for a caller wanting a single body.
    pub fn merged(&self) -> Mesh {
        let mut merged = Mesh::new();
        for part in &self.parts {
            merged.append(&part.mesh);
        }
        merged
    }
}

/// Read `path`, detecting the format by content first and extension second.
pub fn read(path: &std::path::Path, progress: Progress<'_>) -> Result<Model, ImportError> {
    let bytes = std::fs::read(path).map_err(|e| ImportError::Io(format!("{e} ({})", path.display())))?;
    read_bytes(&bytes, Format::from_path(path), progress)
}

/// Read a file already in memory. `named` is the format its name claimed: it decides OBJ, which has
/// no header, and is otherwise used only when the content says nothing.
pub fn read_bytes(bytes: &[u8], named: Option<Format>, progress: Progress<'_>) -> Result<Model, ImportError> {
    if !progress(0.0) {
        return Err(ImportError::Cancelled);
    }
    if bytes.is_empty() {
        return Err(ImportError::Empty);
    }
    let format = Format::sniff(bytes).or(named).ok_or_else(|| {
        ImportError::Unsupported(match named {
            Some(format) => format!("The file is named as {} and does not hold one.", format.label()),
            None => "The file is not one of the model formats Simple 3D reads.".to_string(),
        })
    })?;
    let model = match format {
        Format::ThreeMf => three_mf::read(bytes, progress)?,
        Format::Stl => stl::read(bytes, progress)?,
        Format::Obj => obj::read(bytes, progress)?,
        Format::Ply => ply::read(bytes, progress)?,
    };
    // Files without triangles are refused here, once for every format.
    if model.triangle_count() == 0 {
        return Err(ImportError::Empty);
    }
    progress(1.0);
    Ok(model)
}

/// Text from bytes that should be text, lossily, so a stray byte in a comment does not reject
/// otherwise readable geometry.
pub(crate) fn text(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(bytes)
}

/// Report progress, turning a cancellation into the readers' common error.
pub(crate) fn step(progress: &mut Progress<'_>, fraction: f32) -> Result<(), ImportError> {
    if progress(fraction.clamp(0.0, 1.0)) {
        Ok(())
    } else {
        Err(ImportError::Cancelled)
    }
}
