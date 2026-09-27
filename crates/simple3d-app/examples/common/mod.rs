//! What the example programs share. Each uses only some of it.
#![allow(dead_code)]

use simple3d_geom::primitives as gen;
use simple3d_geom::reassemble::Assembly;
use simple3d_geom::{BooleanOp, Mesh, Vec3};
use std::path::Path;

/// Write `mesh` to `path` as one millimetre 3MF object, compressed, as the application does.
pub fn write_3mf(path: &Path, mesh: &Mesh) -> Result<(), simple3d_export::ExportError> {
    let options = simple3d_export::Options {
        format: simple3d_export::Format::ThreeMf,
        scale: 1.0,
        unit: simple3d_export::Unit3mf::Millimeter,
        allow_invalid: false,
        // One object, so the import is one mesh node to reassemble.
        bodies: simple3d_export::BodyMode::One,
        compress: true,
    };
    simple3d_export::write(path, mesh, &options, &mut |_: f32| true)
}

/// Write `model` of `bodies` bodies to `path`, creating its directory, and read it back as the
/// application does. `None`, with the failure printed, if it could not be written.
pub fn write_and_read_back(path: &Path, model: &Mesh, bodies: usize) -> Option<simple3d_import::Model> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create the output directory");
    }
    if let Err(e) = write_3mf(path, model) {
        println!("FAILED {}: {e}", path.display());
        return None;
    }
    println!("wrote {} -- {bodies} bodies, {} triangles", path.display(), model.triangle_count());
    let read = simple3d_import::read(path, &mut |_: f32| true).expect("the file reads back");
    println!("read back as {} object(s), {} triangles", read.parts.len(), read.triangle_count());
    Some(read)
}

/// How many bodies came back as each shape, most first.
pub fn tally(found: &Assembly) -> Vec<(&str, usize)> {
    let mut counted: Vec<(&str, usize)> = Vec::new();
    for part in &found.parts {
        match counted.iter_mut().find(|(label, _)| *label == part.shape.label()) {
            Some((_, count)) => *count += 1,
            None => counted.push((part.shape.label(), 1)),
        }
    }
    counted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
    counted
}

/// A square plate with a hole: every corner lies on its bounding box, so a corners-only measure
/// would call it a solid box.
pub fn drilled(side: f64, thickness: f64, hole: f64) -> Mesh {
    let plate = gen::box_mesh(side, side, thickness);
    let drill = gen::cylinder_mesh(hole, hole, 40.0, 48);
    simple3d_geom::evaluate_boolean(BooleanOp::Difference, &[plate, drill])
}

/// Two boxes welded into an L, which no single primitive describes: an upright moved along X and a
/// foot moved along Z by `shift`.
pub fn bracket(upright: [f64; 3], foot: [f64; 3], shift: [f64; 2]) -> Mesh {
    let upright = gen::box_mesh(upright[0], upright[1], upright[2]).translated(Vec3::new(shift[0], 0.0, 0.0));
    let foot = gen::box_mesh(foot[0], foot[1], foot[2]).translated(Vec3::new(0.0, 0.0, shift[1]));
    simple3d_geom::evaluate_boolean(BooleanOp::Union, &[upright, foot])
}
