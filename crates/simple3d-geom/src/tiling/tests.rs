mod cutting;
mod grid;
mod plan;
mod preview;

use super::*;
use crate::mesh::Mesh;
use crate::primitives::box_mesh;
use crate::vec3::Vec3;

fn never() -> bool {
    false
}

fn cut_box(tiling: &Tiling) -> Vec<Mesh> {
    cut(&box_mesh(30.0, 30.0, 10.0), tiling, &|| {}, &never).expect("nothing abandoned it")
}

/// The bounds of the 30 x 30 x 10 box every test here cuts.
fn box_bounds() -> (Vec3, Vec3) {
    box_mesh(30.0, 30.0, 10.0).bounds().expect("a box has bounds")
}
