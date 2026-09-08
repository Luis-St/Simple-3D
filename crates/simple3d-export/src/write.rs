//! Writing an export to disk: what the bodies are, and the temporary file
//! that becomes the real one only once it is whole.

use super::*;
use simple3d_geom::Mesh;
use std::io::Write;
use std::path::{Path, PathBuf};

/// One object of an export: a mesh and the name of the node it came from.
///
/// A single-body export is one unnamed part, which is what [`write`] makes;
/// [`write_parts`] is what an export that keeps its objects apart passes
/// several of. The name only reaches the file for a format that has somewhere
/// to put it.
#[derive(Clone, Copy, Debug)]
pub struct Part<'a> {
    pub name: &'a str,
    pub mesh: &'a Mesh,
}

impl<'a> Part<'a> {
    /// The whole export as one part, with no name to give it.
    pub fn whole(mesh: &'a Mesh) -> Part<'a> {
        Part { name: "", mesh }
    }
}

/// Write `mesh` to `path` as a single body. The mesh is welded and scaled
/// first, verified unless the caller opted out, then written to a temporary
/// file and renamed into place.
pub fn write(path: &Path, mesh: &Mesh, options: &Options, progress: Progress<'_>) -> Result<(), ExportError> {
    write_one(path, mesh, options, progress)
}

/// Write `parts` to `path`, each as its own object where the format and the
/// options both allow it (issue 58) and merged into one body otherwise.
///
/// Separating them is not just a matter of how the file is laid out: merged,
/// the parts are welded together and verified as one surface, so two objects
/// that touch are a single watertight solid. Kept apart, each one is welded and
/// verified on its own -- which is stricter, since a part that is only closed
/// because its neighbour fills a gap is now reported.
pub fn write_parts(
    path: &Path,
    parts: &[Part<'_>],
    options: &Options,
    progress: Progress<'_>,
) -> Result<(), ExportError> {
    let separate = options.bodies.separates() && options.format.keeps_objects_separate();
    if !separate || parts.len() < 2 {
        let mut merged = Mesh::new();
        for part in parts {
            merged.append(part.mesh);
        }
        return write_one(path, &merged, options, progress);
    }

    if parts.iter().all(|part| part.mesh.triangle_count() == 0) {
        return Err(ExportError::Empty);
    }
    if !progress(0.0) {
        return Err(ExportError::Cancelled);
    }

    let mut prepared: Vec<(String, Mesh)> = Vec::with_capacity(parts.len());
    let mut problems = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if part.mesh.triangle_count() == 0 {
            continue;
        }
        let mesh = prepare(part.mesh, options);
        if !options.allow_invalid {
            // The node's own name in front of every problem: with several
            // objects in one file, "the mesh is not watertight" on its own does
            // not say which one to go and fix.
            problems.extend(verify(&mesh).into_iter().map(|problem| format!("{}: {problem}", part.name)));
        }
        prepared.push((part.name.to_string(), mesh));
        if !progress(0.2 * ((i + 1) as f32 / parts.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    if !problems.is_empty() {
        return Err(ExportError::Invalid(problems));
    }

    let borrowed: Vec<Part<'_>> = prepared.iter().map(|(name, mesh)| Part { name, mesh }).collect();
    let bytes = three_mf(&borrowed, options, progress)?;
    if !progress(0.9) {
        return Err(ExportError::Cancelled);
    }
    write_atomically(path, &bytes)?;
    progress(1.0);
    Ok(())
}

/// Welding makes the vertex count meaningful for the indexed formats and is
/// what lets the manifold check see a connected surface; the export scale is
/// applied in the same pass, so nothing downstream has to know about it.
pub(crate) fn prepare(mesh: &Mesh, options: &Options) -> Mesh {
    let mut prepared = mesh.weld();
    if (options.scale - 1.0).abs() > f64::EPSILON {
        let scale = options.scale;
        for p in prepared.positions.iter_mut() {
            *p = *p * scale;
        }
    }
    prepared
}

pub(crate) fn write_one(
    path: &Path,
    mesh: &Mesh,
    options: &Options,
    progress: Progress<'_>,
) -> Result<(), ExportError> {
    if mesh.triangle_count() == 0 {
        return Err(ExportError::Empty);
    }
    if !progress(0.0) {
        return Err(ExportError::Cancelled);
    }

    let prepared = prepare(mesh, options);
    if !progress(0.1) {
        return Err(ExportError::Cancelled);
    }

    if !options.allow_invalid {
        let problems = verify(&prepared);
        if !problems.is_empty() {
            return Err(ExportError::Invalid(problems));
        }
    }
    if !progress(0.2) {
        return Err(ExportError::Cancelled);
    }

    let bytes = match options.format {
        Format::ThreeMf => three_mf(&[Part::whole(&prepared)], options, progress)?,
        Format::StlBinary => stl_binary(&prepared, progress)?,
        Format::StlAscii => stl_ascii(&prepared, progress)?,
        Format::Obj => obj(&prepared, progress)?,
        Format::PlyBinary => ply_binary(&prepared, progress)?,
        Format::PlyAscii => ply_ascii(&prepared, progress)?,
    };
    if !progress(0.9) {
        return Err(ExportError::Cancelled);
    }

    write_atomically(path, &bytes)?;
    progress(1.0);
    Ok(())
}

/// Write via a temporary sibling and rename, so a failure part-way through
/// cannot leave a truncated file where the user expects a valid one, and an
/// existing file is only replaced once the new one is complete.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), ExportError> {
    let temp = temp_sibling(path);
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(ExportError::Io(format!("{e} ({})", path.display())));
    }
    Ok(())
}

pub(crate) fn temp_sibling(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "export".into());
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    path.with_file_name(format!(".{name}.{stamp}.part"))
}
