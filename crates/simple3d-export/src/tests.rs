mod atomic;
mod bodies;
mod colour;
mod compression;
mod formats;
mod reporting;
mod verify;

use super::*;
use simple3d_geom::primitives;
use simple3d_geom::Mesh;
use std::path::PathBuf;

fn plate() -> Mesh {
    primitives::box_mesh(40.0, 20.0, 4.0)
}

fn no_progress() -> impl FnMut(f32) -> bool {
    |_| true
}

/// The model document inside a 3MF package, as a reader gets it: the entry is
/// compressed by default, so looking for XML in the file's own bytes finds
/// nothing. Read with the workspace's own reader, which is what makes these
/// tests assertions about the file a slicer opens.
fn model_document(package: &[u8]) -> String {
    let archive = simple3d_import::unzip::Archive::open(package).expect("the export wrote a package");
    let part = archive
        .read_by(|name| name == "3D/3dmodel.model")
        .expect("the package holds a model part")
        .expect("the model part is readable");
    String::from_utf8(part).expect("the model document is text")
}

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "simple3d-export-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
