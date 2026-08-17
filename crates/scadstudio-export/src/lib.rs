//! Mesh export (spec section 9): 3MF, STL, OBJ and PLY, with binary variants
//! where the format has one.
//!
//! Three things the spec insists on and this module implements:
//!
//! * **Verify before writing.** A mesh that is not watertight, manifold and
//!   consistently wound with outward normals is reported -- naming the node
//!   responsible -- rather than written out to fail in a slicer later.
//! * **No partial file.** Everything is written to a sibling temporary file and
//!   renamed into place only once it is complete, so a cancelled or failed
//!   export leaves nothing behind.
//! * **Cancellable with progress.** The caller passes a callback that reports
//!   progress and returns `false` to cancel.

pub mod zip;

use scadstudio_geom::{Mesh, Vec3};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// The default, because it records units.
    ThreeMf,
    StlBinary,
    StlAscii,
    Obj,
    PlyBinary,
    PlyAscii,
}

impl Format {
    pub const ALL: [Format; 6] = [
        Format::ThreeMf,
        Format::StlBinary,
        Format::StlAscii,
        Format::Obj,
        Format::PlyBinary,
        Format::PlyAscii,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Format::ThreeMf => "3MF",
            Format::StlBinary => "STL (binary)",
            Format::StlAscii => "STL (ASCII)",
            Format::Obj => "OBJ",
            Format::PlyBinary => "PLY (binary)",
            Format::PlyAscii => "PLY (ASCII)",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::ThreeMf => "3mf",
            Format::StlBinary | Format::StlAscii => "stl",
            Format::Obj => "obj",
            Format::PlyBinary | Format::PlyAscii => "ply",
        }
    }

    /// A stable identifier for remembering the user's last choice.
    pub fn id(self) -> &'static str {
        match self {
            Format::ThreeMf => "3mf",
            Format::StlBinary => "stl_binary",
            Format::StlAscii => "stl_ascii",
            Format::Obj => "obj",
            Format::PlyBinary => "ply_binary",
            Format::PlyAscii => "ply_ascii",
        }
    }

    pub fn from_id(id: &str) -> Option<Format> {
        Format::ALL.iter().copied().find(|f| f.id() == id)
    }

    /// Whether the format records the unit its numbers are in. For the ones that
    /// do not, the export dialog states the assumed unit instead.
    pub fn carries_units(self) -> bool {
        self == Format::ThreeMf
    }
}

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
}

impl Default for Options {
    fn default() -> Self {
        Options { format: Format::ThreeMf, scale: 1.0, unit: Unit3mf::Millimeter, allow_invalid: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Unit3mf {
    #[default]
    Millimeter,
    Centimeter,
    Meter,
    Inch,
}

impl Unit3mf {
    fn as_str(self) -> &'static str {
        match self {
            Unit3mf::Millimeter => "millimeter",
            Unit3mf::Centimeter => "centimeter",
            Unit3mf::Meter => "meter",
            Unit3mf::Inch => "inch",
        }
    }
}

/// Why an export did not happen. Every variant carries the specific reason, so
/// the dialog never has to show a generic message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportError {
    Empty,
    /// Verification found problems. Each string names what is wrong; the caller
    /// prefixes the node responsible.
    Invalid(Vec<String>),
    Cancelled,
    Io(String),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Empty => write!(
                f,
                "There is nothing to export: the scene has no visible geometry."
            ),
            ExportError::Invalid(problems) => {
                writeln!(f, "The mesh is not valid for 3D printing, so nothing was written:")?;
                for problem in problems {
                    writeln!(f, "  - {problem}")?;
                }
                write!(f, "Fix the reported nodes, or export anyway if you know what you are doing.")
            }
            ExportError::Cancelled => write!(f, "Export cancelled. No file was written."),
            ExportError::Io(why) => write!(f, "Writing the file failed: {why}"),
        }
    }
}

impl std::error::Error for ExportError {}

/// Progress reporting and cancellation. Return `false` to cancel.
pub type Progress<'a> = &'a mut dyn FnMut(f32) -> bool;

/// Check a mesh is fit to write: closed, edge-manifold, consistently wound, and
/// with normals facing outward. Returns one description per problem found.
pub fn verify(mesh: &Mesh) -> Vec<String> {
    let mut problems = Vec::new();
    if mesh.triangle_count() == 0 {
        problems.push("the mesh has no triangles".to_string());
        return problems;
    }
    if let Some(issue) = mesh.manifold_issue() {
        problems.push(format!("the mesh is not watertight and manifold ({issue})"));
    }
    let volume = signed_volume(mesh);
    if volume <= 0.0 {
        problems.push(format!(
            "the triangles are wound inward (signed volume {volume:.3} mm3); normals would point into the solid"
        ));
    }
    for (i, tri) in mesh.indices.iter().enumerate() {
        let normal = mesh.triangle_normal(*tri);
        if normal.length() < 0.5 {
            problems.push(format!("triangle {i} has no area, so it has no normal"));
            break;
        }
    }
    problems
}

/// Six times the signed volume is the sum of the triangles' scalar triple
/// products; positive means the winding is outward for a closed mesh.
pub fn signed_volume(mesh: &Mesh) -> f64 {
    let mut total = 0.0;
    for tri in &mesh.indices {
        let a = mesh.positions[tri[0] as usize];
        let b = mesh.positions[tri[1] as usize];
        let c = mesh.positions[tri[2] as usize];
        total += a.dot(b.cross(c));
    }
    total / 6.0
}

/// Write `mesh` to `path`. The mesh is welded and scaled first, verified unless
/// the caller opted out, then written to a temporary file and renamed into place.
pub fn write(path: &Path, mesh: &Mesh, options: &Options, progress: Progress<'_>) -> Result<(), ExportError> {
    if mesh.triangle_count() == 0 {
        return Err(ExportError::Empty);
    }
    if !progress(0.0) {
        return Err(ExportError::Cancelled);
    }

    // Welding makes the vertex count meaningful for the indexed formats and is
    // what lets the manifold check see a connected surface.
    let mut prepared = mesh.weld();
    if (options.scale - 1.0).abs() > f64::EPSILON {
        let scale = options.scale;
        for p in prepared.positions.iter_mut() {
            *p = *p * scale;
        }
    }
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
        Format::ThreeMf => three_mf(&prepared, options, progress)?,
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
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), ExportError> {
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

fn temp_sibling(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "export".into());
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    path.with_file_name(format!(".{name}.{stamp}.part"))
}

/// Trim a coordinate to a fixed number of decimals without floating-point
/// noise, matching what the property editor shows the user.
fn coord(v: f64) -> String {
    let mut s = format!("{v:.6}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

fn three_mf(mesh: &Mesh, options: &Options, progress: Progress<'_>) -> Result<Vec<u8>, ExportError> {
    let mut model = String::with_capacity(mesh.positions.len() * 48 + mesh.indices.len() * 40);
    model.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    model.push_str(&format!(
        "<model unit=\"{}\" xml:lang=\"en-US\" \
         xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n",
        options.unit.as_str()
    ));
    model.push_str(" <resources>\n  <object id=\"1\" type=\"model\">\n   <mesh>\n    <vertices>\n");
    for (i, p) in mesh.positions.iter().enumerate() {
        model.push_str(&format!(
            "     <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
            coord(p.x),
            coord(p.y),
            coord(p.z)
        ));
        if i % 4096 == 0 && !progress(0.2 + 0.4 * (i as f32 / mesh.positions.len().max(1) as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    model.push_str("    </vertices>\n    <triangles>\n");
    for (i, t) in mesh.indices.iter().enumerate() {
        model.push_str(&format!("     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>\n", t[0], t[1], t[2]));
        if i % 4096 == 0 && !progress(0.6 + 0.3 * (i as f32 / mesh.indices.len().max(1) as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    model.push_str("    </triangles>\n   </mesh>\n  </object>\n </resources>\n");
    model.push_str(" <build>\n  <item objectid=\"1\"/>\n </build>\n</model>\n");

    const CONTENT_TYPES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
        <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
        <Default Extension=\"rels\" \
        ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
        <Default Extension=\"model\" \
        ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/>\
        </Types>\n";
    const RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
        <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
        <Relationship Id=\"rel0\" Target=\"/3D/3dmodel.model\" \
        Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/>\
        </Relationships>\n";

    let mut archive = zip::ZipWriter::new();
    archive.add("[Content_Types].xml", CONTENT_TYPES.as_bytes());
    archive.add("_rels/.rels", RELS.as_bytes());
    archive.add("3D/3dmodel.model", model.as_bytes());
    Ok(archive.finish())
}

fn stl_binary(mesh: &Mesh, progress: Progress<'_>) -> Result<Vec<u8>, ExportError> {
    let mut out = Vec::with_capacity(84 + mesh.indices.len() * 50);
    let mut header = [0u8; 80];
    let banner = b"Exported by ScadStudio";
    header[..banner.len()].copy_from_slice(banner);
    out.extend_from_slice(&header);
    out.extend_from_slice(&(mesh.indices.len() as u32).to_le_bytes());
    for (i, tri) in mesh.indices.iter().enumerate() {
        let n = mesh.triangle_normal(*tri);
        push_f32(&mut out, n);
        for &vertex in tri {
            push_f32(&mut out, mesh.positions[vertex as usize]);
        }
        out.extend_from_slice(&0u16.to_le_bytes()); // attribute byte count
        if i % 8192 == 0 && !progress(0.2 + 0.7 * (i as f32 / mesh.indices.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    Ok(out)
}

fn push_f32(out: &mut Vec<u8>, v: Vec3) {
    out.extend_from_slice(&(v.x as f32).to_le_bytes());
    out.extend_from_slice(&(v.y as f32).to_le_bytes());
    out.extend_from_slice(&(v.z as f32).to_le_bytes());
}

fn stl_ascii(mesh: &Mesh, progress: Progress<'_>) -> Result<Vec<u8>, ExportError> {
    let mut out = String::with_capacity(mesh.indices.len() * 180);
    out.push_str("solid scadstudio\n");
    for (i, tri) in mesh.indices.iter().enumerate() {
        let n = mesh.triangle_normal(*tri);
        out.push_str(&format!("  facet normal {} {} {}\n", coord(n.x), coord(n.y), coord(n.z)));
        out.push_str("    outer loop\n");
        for &vertex in tri {
            let p = mesh.positions[vertex as usize];
            out.push_str(&format!("      vertex {} {} {}\n", coord(p.x), coord(p.y), coord(p.z)));
        }
        out.push_str("    endloop\n  endfacet\n");
        if i % 4096 == 0 && !progress(0.2 + 0.7 * (i as f32 / mesh.indices.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    out.push_str("endsolid scadstudio\n");
    Ok(out.into_bytes())
}

fn obj(mesh: &Mesh, progress: Progress<'_>) -> Result<Vec<u8>, ExportError> {
    let mut out = String::with_capacity(mesh.positions.len() * 32 + mesh.indices.len() * 24);
    out.push_str("# Exported by ScadStudio\n# Units: millimetres\n");
    out.push_str("o scadstudio\n");
    for (i, p) in mesh.positions.iter().enumerate() {
        out.push_str(&format!("v {} {} {}\n", coord(p.x), coord(p.y), coord(p.z)));
        if i % 8192 == 0 && !progress(0.2 + 0.4 * (i as f32 / mesh.positions.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    for (i, t) in mesh.indices.iter().enumerate() {
        // OBJ indices are 1-based.
        out.push_str(&format!("f {} {} {}\n", t[0] + 1, t[1] + 1, t[2] + 1));
        if i % 8192 == 0 && !progress(0.6 + 0.3 * (i as f32 / mesh.indices.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    Ok(out.into_bytes())
}

fn ply_header(mesh: &Mesh, binary: bool) -> String {
    let format = if binary { "binary_little_endian 1.0" } else { "ascii 1.0" };
    format!(
        "ply\nformat {format}\ncomment Exported by ScadStudio\ncomment Units: millimetres\n\
         element vertex {}\nproperty double x\nproperty double y\nproperty double z\n\
         element face {}\nproperty list uchar uint vertex_indices\nend_header\n",
        mesh.positions.len(),
        mesh.indices.len()
    )
}

fn ply_ascii(mesh: &Mesh, progress: Progress<'_>) -> Result<Vec<u8>, ExportError> {
    let mut out = ply_header(mesh, false);
    for (i, p) in mesh.positions.iter().enumerate() {
        out.push_str(&format!("{} {} {}\n", coord(p.x), coord(p.y), coord(p.z)));
        if i % 8192 == 0 && !progress(0.2 + 0.4 * (i as f32 / mesh.positions.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    for (i, t) in mesh.indices.iter().enumerate() {
        out.push_str(&format!("3 {} {} {}\n", t[0], t[1], t[2]));
        if i % 8192 == 0 && !progress(0.6 + 0.3 * (i as f32 / mesh.indices.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    Ok(out.into_bytes())
}

fn ply_binary(mesh: &Mesh, progress: Progress<'_>) -> Result<Vec<u8>, ExportError> {
    let mut out = ply_header(mesh, true).into_bytes();
    for (i, p) in mesh.positions.iter().enumerate() {
        out.extend_from_slice(&p.x.to_le_bytes());
        out.extend_from_slice(&p.y.to_le_bytes());
        out.extend_from_slice(&p.z.to_le_bytes());
        if i % 8192 == 0 && !progress(0.2 + 0.4 * (i as f32 / mesh.positions.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    for (i, t) in mesh.indices.iter().enumerate() {
        out.push(3);
        for &vertex in t {
            out.extend_from_slice(&vertex.to_le_bytes());
        }
        if i % 8192 == 0 && !progress(0.6 + 0.3 * (i as f32 / mesh.indices.len() as f32)) {
            return Err(ExportError::Cancelled);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scadstudio_geom::primitives;

    fn plate() -> Mesh {
        primitives::box_mesh(40.0, 20.0, 4.0)
    }

    fn no_progress() -> impl FnMut(f32) -> bool {
        |_| true
    }

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "scadstudio-export-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_valid_mesh_passes_verification() {
        assert!(verify(&plate()).is_empty());
        assert!(signed_volume(&plate().weld()) > 0.0);
    }

    #[test]
    fn inward_winding_is_caught_before_anything_is_written() {
        let mut flipped = plate();
        flipped.flip_winding();
        let problems = verify(&flipped);
        assert!(problems.iter().any(|p| p.contains("wound inward")), "{problems:?}");
    }

    #[test]
    fn an_open_mesh_is_caught() {
        let mut open = plate();
        open.indices.pop();
        let problems = verify(&open);
        assert!(problems.iter().any(|p| p.contains("watertight")), "{problems:?}");
    }

    #[test]
    fn export_refuses_an_invalid_mesh_and_writes_nothing() {
        let mut open = plate();
        open.indices.pop();
        let path = temp_dir().join("invalid.stl");
        let mut cb = no_progress();
        let err = write(&path, &open, &Options { format: Format::StlBinary, ..Default::default() }, &mut cb)
            .unwrap_err();
        assert!(matches!(err, ExportError::Invalid(_)));
        assert!(err.to_string().contains("watertight"));
        assert!(!path.exists(), "a file was written for a mesh that failed verification");
    }

    #[test]
    fn export_anyway_is_possible_once_the_user_has_been_told() {
        let mut open = plate();
        open.indices.pop();
        let path = temp_dir().join("anyway.stl");
        let mut cb = no_progress();
        let options = Options { format: Format::StlBinary, allow_invalid: true, ..Default::default() };
        write(&path, &open, &options, &mut cb).unwrap();
        assert!(path.exists());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn every_format_writes_a_file_with_the_expected_shape() {
        let mesh = plate();
        let welded = mesh.weld();
        for format in Format::ALL {
            let path = temp_dir().join(format!("plate.{}", format.id()));
            let mut cb = no_progress();
            write(&path, &mesh, &Options { format, ..Default::default() }, &mut cb).unwrap();
            let bytes = std::fs::read(&path).unwrap();
            assert!(!bytes.is_empty(), "{format:?} wrote nothing");

            match format {
                Format::ThreeMf => {
                    assert_eq!(&bytes[0..2], b"PK");
                    let text = String::from_utf8_lossy(&bytes);
                    assert!(text.contains("unit=\"millimeter\""), "3MF did not record its unit");
                    assert!(text.contains("<triangle v1="));
                    assert!(text.contains("[Content_Types].xml"));
                    assert!(text.contains("3D/3dmodel.model"));
                }
                Format::StlBinary => {
                    let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]);
                    assert_eq!(count as usize, welded.indices.len());
                    assert_eq!(bytes.len(), 84 + welded.indices.len() * 50);
                }
                Format::StlAscii => {
                    let text = String::from_utf8(bytes).unwrap();
                    assert!(text.starts_with("solid "));
                    assert!(text.trim_end().ends_with("endsolid scadstudio"));
                    assert_eq!(text.matches("facet normal").count(), welded.indices.len());
                }
                Format::Obj => {
                    let text = String::from_utf8(bytes).unwrap();
                    assert_eq!(text.lines().filter(|l| l.starts_with("v ")).count(), welded.positions.len());
                    assert_eq!(text.lines().filter(|l| l.starts_with("f ")).count(), welded.indices.len());
                    // 1-based indices, and none out of range.
                    for line in text.lines().filter(|l| l.starts_with("f ")) {
                        for field in line[2..].split_whitespace() {
                            let i: usize = field.parse().unwrap();
                            assert!(i >= 1 && i <= welded.positions.len());
                        }
                    }
                }
                Format::PlyAscii => {
                    let text = String::from_utf8(bytes).unwrap();
                    assert!(text.starts_with("ply\nformat ascii 1.0\n"));
                    assert!(text.contains(&format!("element vertex {}", welded.positions.len())));
                    assert_eq!(text.lines().filter(|l| l.starts_with("3 ")).count(), welded.indices.len());
                }
                Format::PlyBinary => {
                    let text = String::from_utf8_lossy(&bytes);
                    assert!(text.starts_with("ply\nformat binary_little_endian 1.0\n"));
                    let header_end = bytes.windows(11).position(|w| w == b"end_header\n").unwrap() + 11;
                    let expected = welded.positions.len() * 24 + welded.indices.len() * 13;
                    assert_eq!(bytes.len() - header_end, expected);
                }
            }
            std::fs::remove_file(&path).unwrap();
        }
    }

    #[test]
    fn the_exported_bounding_box_matches_the_entered_dimensions() {
        // Spec acceptance criteria 1 and 2, read back out of the written file.
        let path = temp_dir().join("dims.obj");
        let mut cb = no_progress();
        write(&path, &plate(), &Options { format: Format::Obj, ..Default::default() }, &mut cb).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let mut lo = [f64::MAX; 3];
        let mut hi = [f64::MIN; 3];
        for line in text.lines().filter(|l| l.starts_with("v ")) {
            for (axis, field) in line[2..].split_whitespace().enumerate() {
                let v: f64 = field.parse().unwrap();
                lo[axis] = lo[axis].min(v);
                hi[axis] = hi[axis].max(v);
            }
        }
        assert_eq!(hi[0] - lo[0], 40.0);
        assert_eq!(hi[1] - lo[1], 20.0);
        assert_eq!(hi[2] - lo[2], 4.0);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn the_scale_factor_multiplies_every_dimension_and_defaults_to_one() {
        let path = temp_dir().join("scaled.stl");
        let mut cb = no_progress();
        let options = Options { format: Format::StlBinary, scale: 2.0, ..Default::default() };
        write(&path, &plate(), &options, &mut cb).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let mut max_x = f32::MIN;
        for i in 0..u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize {
            let base = 84 + i * 50 + 12;
            for v in 0..3 {
                let o = base + v * 12;
                max_x = max_x.max(f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]));
            }
        }
        assert!((max_x - 40.0).abs() < 1e-3, "scale 2.0 should put the far face at 40mm, got {max_x}");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_cancelled_export_leaves_no_file_behind() {
        // Spec acceptance criterion 16.
        let path = temp_dir().join("cancelled.3mf");
        let mut calls = 0;
        let mut cb = |_: f32| {
            calls += 1;
            calls < 3
        };
        let err = write(&path, &plate(), &Options::default(), &mut cb).unwrap_err();
        assert_eq!(err, ExportError::Cancelled);
        assert!(!path.exists(), "cancelling left a file behind");
        // And nothing half-written next to it either.
        let leftovers: Vec<_> = std::fs::read_dir(temp_dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".part"))
            .collect();
        assert!(leftovers.is_empty(), "temporary files left behind: {leftovers:?}");
    }

    #[test]
    fn an_existing_file_survives_a_failed_export() {
        let path = temp_dir().join("existing.stl");
        std::fs::write(&path, b"original contents").unwrap();
        let mut open = plate();
        open.indices.pop();
        let mut cb = no_progress();
        let _ = write(&path, &open, &Options { format: Format::StlBinary, ..Default::default() }, &mut cb);
        assert_eq!(std::fs::read(&path).unwrap(), b"original contents");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn an_empty_scene_is_reported_as_such() {
        let path = temp_dir().join("empty.stl");
        let mut cb = no_progress();
        let err = write(&path, &Mesh::new(), &Options::default(), &mut cb).unwrap_err();
        assert_eq!(err, ExportError::Empty);
        assert!(err.to_string().contains("nothing to export"));
    }

    #[test]
    fn progress_runs_from_zero_to_one() {
        let mut seen: Vec<f32> = Vec::new();
        let mut cb = |p: f32| {
            seen.push(p);
            true
        };
        write(&temp_dir().join("progress.obj"), &plate(), &Options { format: Format::Obj, ..Default::default() }, &mut cb)
            .unwrap();
        assert_eq!(seen.first(), Some(&0.0));
        assert_eq!(seen.last(), Some(&1.0));
        assert!(seen.windows(2).all(|w| w[1] >= w[0] - 1e-6), "progress went backwards: {seen:?}");
        std::fs::remove_file(temp_dir().join("progress.obj")).unwrap();
    }

    #[test]
    fn exporting_the_same_mesh_twice_produces_the_same_bytes() {
        for format in Format::ALL {
            let path = temp_dir().join(format!("stable.{}", format.id()));
            let mut cb = no_progress();
            write(&path, &plate(), &Options { format, ..Default::default() }, &mut cb).unwrap();
            let first = std::fs::read(&path).unwrap();
            write(&path, &plate(), &Options { format, ..Default::default() }, &mut cb).unwrap();
            let second = std::fs::read(&path).unwrap();
            assert_eq!(first, second, "{format:?} is not reproducible");
            std::fs::remove_file(&path).unwrap();
        }
    }

    #[test]
    fn format_ids_round_trip_for_remembering_the_last_choice() {
        for format in Format::ALL {
            assert_eq!(Format::from_id(format.id()), Some(format));
            assert!(!format.extension().is_empty());
            assert!(!format.label().is_empty());
        }
        assert_eq!(Format::from_id("nonsense"), None);
        // Only 3MF records its unit; the dialog states the assumption for the rest.
        assert_eq!(Format::ALL.iter().filter(|f| f.carries_units()).count(), 1);
    }

    #[test]
    fn a_drilled_plate_exports_as_a_watertight_solid() {
        use scadstudio_geom::{evaluate_boolean, BooleanOp};
        let hole = primitives::cylinder_mesh(6.0, 6.0, 20.0, 32).translated(Vec3::new(-8.0, 0.0, 0.0));
        let mesh = evaluate_boolean(BooleanOp::Difference, &[plate(), hole]);
        assert!(verify(&mesh.weld()).is_empty(), "{:?}", verify(&mesh.weld()));
        let path = temp_dir().join("drilled.3mf");
        let mut cb = no_progress();
        write(&path, &mesh, &Options::default(), &mut cb).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() > 500);
        std::fs::remove_file(&path).unwrap();
    }
}
