//! Mesh import (issue 105): every format [`simple3d_export`] writes can be read
//! back in -- 3MF, STL, OBJ and PLY, binary or text.
//!
//! What this crate hands back is triangles and the names they were written
//! under, and nothing else. A file from another program has no parameters to
//! recover -- there is no box of 40 by 20 in an STL, only its surface -- so an
//! import becomes a *stored mesh* body in the scene, the same kind of node a
//! shape becomes when it is converted (issue 80). What it does keep is the
//! structure the file holds: a 3MF's objects, an OBJ's groups and an STL's
//! several solids come back as separate parts rather than as one bag of
//! triangles, which is the other half of the export that can keep its parts.
//!
//! Three things it insists on, mirroring the exporter:
//!
//! * **Say what is wrong.** A file that is not readable is refused with the
//!   reason -- where the parse stopped, which part of a 3MF was missing -- and
//!   never with a generic failure.
//! * **Nothing half-read.** A parse either produces the whole model or an
//!   error; a file that runs out part-way through is not brought in as the half
//!   that happened to be understood.
//! * **Cancellable with progress.** The caller passes the same callback shape
//!   the exporter takes: it reports progress and returns `false` to cancel.

mod error;
pub(crate) use error::malformed;
pub use error::ImportError;
mod format;
pub use format::{Format, Unit};
/// The DEFLATE decoder. Public because the export crate's encoder is tested
/// against it: an encoder that is only checked by its own decoder proves
/// nothing, and these two were written to the same RFC from opposite ends.
pub mod inflate;
mod obj;
mod ply;
mod stl;
#[cfg(test)]
mod tests;
mod three_mf;
/// The package reader. Public alongside [`inflate`] and for the same reason:
/// the export crate writes 3MF packages and its tests have to read them back
/// the way a slicer would, rather than by looking for XML in the raw bytes --
/// which stopped saying anything the moment those bytes were compressed.
pub mod unzip;
mod xml;

use simple3d_geom::Mesh;

/// Progress reporting and cancellation, the same contract the exporter's is:
/// return `false` to cancel.
pub type Progress<'a> = &'a mut dyn FnMut(f32) -> bool;

/// One object of a file: its triangles, and the name it was written under.
///
/// The name is empty where the format has nowhere to put one -- an STL is a
/// bag of triangles and says nothing about what they are -- and the caller
/// names the node after the file in that case.
#[derive(Clone, Debug)]
pub struct Part {
    pub name: String,
    pub mesh: Mesh,
}

/// What a file held.
#[derive(Clone, Debug)]
pub struct Model {
    pub format: Format,
    /// The unit the file stated, for a format that records one. The positions
    /// in `parts` are already millimetres -- this is what they were converted
    /// *from*, so an import can say so.
    pub unit: Option<Unit>,
    pub parts: Vec<Part>,
}

impl Model {
    pub fn triangle_count(&self) -> usize {
        self.parts.iter().map(|part| part.mesh.triangle_count()).sum()
    }

    /// Everything in one mesh, for a caller that wants a single body.
    pub fn merged(&self) -> Mesh {
        let mut merged = Mesh::new();
        for part in &self.parts {
            merged.append(&part.mesh);
        }
        merged
    }
}

/// Read `path`. The format is decided by looking at the bytes first and at the
/// extension second, so a renamed file still reads as what it is.
pub fn read(path: &std::path::Path, progress: Progress<'_>) -> Result<Model, ImportError> {
    let bytes = std::fs::read(path).map_err(|e| ImportError::Io(format!("{e} ({})", path.display())))?;
    read_bytes(&bytes, Format::from_path(path), progress)
}

/// Read a file already in memory. `named` is the format its name claimed, if
/// any: it decides an OBJ, which has no header to recognise it by, and is
/// otherwise only consulted when the content says nothing.
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
    // A file with no triangles in it is refused here rather than by every
    // reader: an empty object list, an OBJ of vertices with no faces and an STL
    // whose triangle count is zero are all the same answer to the user.
    if model.triangle_count() == 0 {
        return Err(ImportError::Empty);
    }
    progress(1.0);
    Ok(model)
}

/// Text out of bytes that are supposed to be text. Not `String::from_utf8`:
/// an OBJ or an STL written on a machine with a different code page is a file
/// whose *numbers* are still plain ASCII, and refusing the whole model over a
/// stray byte in a comment would be refusing geometry that reads perfectly.
pub(crate) fn text(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(bytes)
}

/// Report progress, and turn a cancellation into the error every reader
/// returns for one.
pub(crate) fn step(progress: &mut Progress<'_>, fraction: f32) -> Result<(), ImportError> {
    if progress(fraction.clamp(0.0, 1.0)) {
        Ok(())
    } else {
        Err(ImportError::Cancelled)
    }
}
