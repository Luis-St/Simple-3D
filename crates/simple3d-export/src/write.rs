//! Writing an export to disk: its bodies, and a temporary file renamed only once complete.

use super::*;
use simple3d_geom::Mesh;
use std::io::Write;
use std::path::{Path, PathBuf};

/// One export object: a mesh and its node's name. [`write`] makes one unnamed part; [`write_parts`]
/// takes several. The name reaches the file only where the format has room for it.
#[derive(Clone, Copy, Debug)]
pub struct Part<'a> {
    pub name: &'a str,
    pub mesh: &'a Mesh,
}

impl<'a> Part<'a> {
    /// The whole export as one unnamed part.
    pub fn whole(mesh: &'a Mesh) -> Part<'a> {
        Part { name: "", mesh }
    }
}

/// Write `mesh` to `path` as a single body: welded, scaled, verified unless opted out, then written
/// to a temporary file and renamed.
pub fn write(path: &Path, mesh: &Mesh, options: &Options, progress: Progress<'_>) -> Result<(), ExportError> {
    write_one(path, mesh, options, progress)
}

/// Write `parts` to `path`, as separate objects where format and options allow (issue 58), else
/// merged. Separate parts are welded and verified individually, which is stricter than merging.
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
            // Prefix each problem with the node's name, so it says which object to fix.
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

/// Weld (meaningful vertex counts, connected surface for the manifold check) and apply the export
/// scale in one pass.
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

/// Write via a temporary sibling and rename, so a failure never leaves a truncated file and an
/// existing one is only replaced once the new one is complete.
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
