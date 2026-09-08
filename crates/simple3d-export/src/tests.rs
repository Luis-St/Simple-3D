mod atomic;
mod bodies;
mod colour;
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

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "simple3d-export-test-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
