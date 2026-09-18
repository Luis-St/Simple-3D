mod deflate;
mod detect;
mod obj;
mod ply;
mod roundtrip;
mod stl;
mod three_mf;

use super::*;
use simple3d_geom::{primitives, Vec3};

fn plate() -> Mesh {
    primitives::box_mesh(40.0, 20.0, 4.0)
}

fn no_progress() -> impl FnMut(f32) -> bool {
    |_| true
}

/// The format a file of this kind would be *named* as, which is how the
/// application knows an OBJ: it has no header to be recognised by.
fn named_as(format: simple3d_export::Format) -> Option<Format> {
    Format::from_path(std::path::Path::new(&format!("model.{}", format.extension())))
}

/// Read bytes that are not going through a file, which is most of these tests:
/// what a reader has to cope with is a file's *content*, and writing each one
/// to disk first would only test the filesystem.
fn read_all(bytes: &[u8], named: Option<Format>) -> Result<Model, ImportError> {
    read_bytes(bytes, named, &mut no_progress())
}

/// What the exporter writes for `mesh` in `format`, in memory.
fn exported(mesh: &Mesh, format: simple3d_export::Format) -> Vec<u8> {
    let dir = std::env::temp_dir().join(format!(
        "simple3d-import-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("model.{}", format.id()));
    let options = simple3d_export::Options { format, ..Default::default() };
    simple3d_export::write(&path, mesh, &options, &mut no_progress()).unwrap();
    std::fs::read(&path).unwrap()
}

/// The same for an export of several named bodies, which only 3MF keeps apart.
fn exported_parts(parts: &[(&str, Mesh)], format: simple3d_export::Format) -> Vec<u8> {
    let dir = std::env::temp_dir().join(format!(
        "simple3d-import-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("parts.{}", format.id()));
    let borrowed: Vec<simple3d_export::Part<'_>> =
        parts.iter().map(|(name, mesh)| simple3d_export::Part { name, mesh }).collect();
    let options =
        simple3d_export::Options { format, bodies: simple3d_export::BodyMode::TopLevel, ..Default::default() };
    simple3d_export::write_parts(&path, &borrowed, &options, &mut no_progress()).unwrap();
    std::fs::read(&path).unwrap()
}

/// How far apart two boxes' corners are, for comparing a mesh with the mesh it
/// was written from.
fn bounds_differ(a: &Mesh, b: &Mesh) -> f64 {
    let (alo, ahi) = a.bounds().expect("the first mesh is empty");
    let (blo, bhi) = b.bounds().expect("the second mesh is empty");
    (alo - blo).length().max((ahi - bhi).length())
}

/// A zip entry whose data is deflated, built without a compressor: a DEFLATE
/// stream may carry a *stored* block, which is a length, its complement and the
/// bytes -- so a valid compressed entry can be written by hand.
fn deflate_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x01];
    out.extend_from_slice(&(data.len() as u16).to_le_bytes());
    out.extend_from_slice(&(!(data.len() as u16)).to_le_bytes());
    out.extend_from_slice(data);
    out
}

/// A zip archive whose entries are written with method 8, which is what every
/// 3MF from another program is and what the export crate's own writer -- store
/// only -- cannot produce.
///
/// The CRC is left at zero: nothing in the reader checks it, deliberately, since
/// a 3MF's geometry is verified by the model it parses into rather than by a
/// checksum of the bytes.
fn deflated_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut directory: Vec<u8> = Vec::new();
    let mut count = 0u16;
    for (name, data) in entries {
        let compressed = deflate_stored(data);
        let offset = out.len() as u32;
        out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&8u16.to_le_bytes()); // deflated
        out.extend_from_slice(&0u32.to_le_bytes()); // time and date
        out.extend_from_slice(&0u32.to_le_bytes()); // crc
        out.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&compressed);

        directory.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
        directory.extend_from_slice(&20u16.to_le_bytes()); // version made by
        directory.extend_from_slice(&20u16.to_le_bytes()); // version needed
        directory.extend_from_slice(&0u16.to_le_bytes()); // flags
        directory.extend_from_slice(&8u16.to_le_bytes()); // deflated
        directory.extend_from_slice(&0u32.to_le_bytes()); // time and date
        directory.extend_from_slice(&0u32.to_le_bytes()); // crc
        directory.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        directory.extend_from_slice(&(data.len() as u32).to_le_bytes());
        directory.extend_from_slice(&(name.len() as u16).to_le_bytes());
        directory.extend_from_slice(&0u16.to_le_bytes()); // extra
        directory.extend_from_slice(&0u16.to_le_bytes()); // comment
        directory.extend_from_slice(&0u16.to_le_bytes()); // disk
        directory.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
        directory.extend_from_slice(&0u32.to_le_bytes()); // external attributes
        directory.extend_from_slice(&offset.to_le_bytes());
        directory.extend_from_slice(name.as_bytes());
        count += 1;
    }
    let directory_at = out.len() as u32;
    out.extend_from_slice(&directory);
    out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
    out.extend_from_slice(&0u16.to_le_bytes()); // this disk
    out.extend_from_slice(&0u16.to_le_bytes()); // the disk the directory is on
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&(directory.len() as u32).to_le_bytes());
    out.extend_from_slice(&directory_at.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment
    out
}

/// A 3MF package around one model document, deflated.
fn package(model: &str) -> Vec<u8> {
    deflated_zip(&[
        ("[Content_Types].xml", b"<?xml version=\"1.0\"?><Types/>"),
        ("_rels/.rels", b"<?xml version=\"1.0\"?><Relationships/>"),
        ("3D/3dmodel.model", model.as_bytes()),
    ])
}

/// A 3MF model document holding one tetrahedron, so the vertices and triangles
/// of a test are four and four rather than a page of them.
fn tetrahedron_model(unit: &str, object_attributes: &str, item_attributes: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <model unit=\"{unit}\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n\
         <resources>\n\
         <object id=\"1\" type=\"model\"{object_attributes}><mesh><vertices>\n\
         <vertex x=\"0\" y=\"0\" z=\"0\"/><vertex x=\"10\" y=\"0\" z=\"0\"/>\n\
         <vertex x=\"0\" y=\"10\" z=\"0\"/><vertex x=\"0\" y=\"0\" z=\"10\"/>\n\
         </vertices><triangles>\n\
         <triangle v1=\"0\" v2=\"2\" v3=\"1\"/><triangle v1=\"0\" v2=\"1\" v3=\"3\"/>\n\
         <triangle v1=\"1\" v2=\"2\" v3=\"3\"/><triangle v1=\"0\" v2=\"3\" v3=\"2\"/>\n\
         </triangles></mesh></object>\n\
         </resources>\n\
         <build><item objectid=\"1\"{item_attributes}/></build>\n\
         </model>\n"
    )
}
