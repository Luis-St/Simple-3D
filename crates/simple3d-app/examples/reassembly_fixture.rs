//! Writes the 3MF used to try the reassembly out (issue 108): one object whose
//! mesh holds eighty-odd separate bodies, laid out so that every branch of the
//! recognition has something to be tried on.
//!
//! The bodies are *appended*, never unioned, which is the whole point. A
//! printer file is a bag of surfaces and says nothing about which of them are
//! one solid -- a pin standing in a plate is two closed shells whose triangles
//! happen to touch -- and putting them through the boolean kernel first would
//! weld them into one surface, which is a different model and one the
//! recognition would be right to refuse. Bodies that are *meant* to be one
//! shell (the drilled plate, the L-bracket) go through the kernel on purpose,
//! and come out as the single unrecognisable body they are.
//!
//! What is in it, and what each thing is there to test:
//!
//! * **Two assemblies of parts standing on a plate** -- boxes, cylinders at
//!   several tessellations, a hexagonal prism, a cone, a frustum, a sphere.
//!   Each part touches its plate, so they should come back as one group apiece
//!   with every part named; the two assemblies stand well apart, so they should
//!   *not* be gathered into one.
//! * **A row of shapes standing on their own, turned every which way** -- the
//!   fits have to find an axis rather than assume one, and a body that is not
//!   near a world axis is where that shows.
//! * **A row of shapes nothing here can rebuild** -- a torus, a drilled plate,
//!   an L-bracket, a tube, a capsule, a rounded box. These have to come back as
//!   meshes, unchanged, rather than as the box or cylinder each of them nearly
//!   is. Getting one of these *recognised* is a worse failure than missing a
//!   cylinder.
//! * **Sixty small cubes in a field** -- for the cap. They are the smallest
//!   bodies in the file, so lowering "At most" leaves the big parts as objects
//!   and sweeps the cubes into the one leftover mesh.
//!
//! usage: reassembly_fixture [out.3mf]

use simple3d_geom::primitives as gen;
use simple3d_geom::{BooleanOp, Mesh, Vec3};
use std::path::PathBuf;

fn main() {
    let path = std::env::args().nth(1).map_or_else(|| PathBuf::from("exports/reassembly-test.3mf"), PathBuf::from);
    let mut model = Mesh::new();
    let mut bodies = 0;
    let mut put = |mesh: Mesh, at: Vec3, turn: Vec3| {
        model.append(&mesh.transformed(at, turn));
        bodies += 1;
    };

    // -- assembly one: a plate with four posts and a boss on it ------------
    let plate = Vec3::new(-260.0, 0.0, 0.0);
    put(gen::box_mesh(150.0, 110.0, 8.0), plate + Vec3::new(0.0, 0.0, 4.0), Vec3::ZERO);
    for (x, y) in [(-57.0, -37.0), (57.0, -37.0), (57.0, 37.0), (-57.0, 37.0)] {
        // Sunk two millimetres into the plate, the way a part that is meant to
        // be in contact is modelled.
        put(gen::cylinder_mesh(14.0, 14.0, 40.0, 32), plate + Vec3::new(x, y, 26.0), Vec3::ZERO);
    }
    put(gen::regular_prism_mesh(6, 44.0, 22.0, false), plate + Vec3::new(0.0, 0.0, 17.0), Vec3::ZERO);

    // -- assembly two: a stack that comes to a point -----------------------
    let stack = Vec3::new(-40.0, 0.0, 0.0);
    put(gen::cylinder_mesh(90.0, 90.0, 12.0, 64), stack + Vec3::new(0.0, 0.0, 6.0), Vec3::ZERO);
    put(gen::cone_mesh(78.0, 30.0, 34.0, 64), stack + Vec3::new(0.0, 0.0, 27.0), Vec3::ZERO);
    // Wider than the frustum it stands on, and sunk into it. Matching its rim
    // exactly would put a vertex of each in the same place, and a shared vertex
    // is what "one body" means here: the two would weld into a single
    // unrecognisable solid on the way through the file.
    put(gen::cone_mesh(34.0, 0.0, 28.0, 48), stack + Vec3::new(0.0, 0.0, 56.0), Vec3::ZERO);
    put(gen::ellipsoid_mesh(24.0, 24.0, 24.0, 32), stack + Vec3::new(0.0, 0.0, 78.0), Vec3::ZERO);

    // -- assembly three: the coarse tessellations --------------------------
    let coarse = Vec3::new(160.0, 0.0, 0.0);
    put(gen::box_mesh(120.0, 90.0, 8.0), coarse + Vec3::new(0.0, 0.0, 4.0), Vec3::ZERO);
    // Eight segments: the same triangles a cylinder has, and few enough of them
    // that the shape somebody meant is a prism.
    put(gen::cylinder_mesh(34.0, 34.0, 30.0, 8), coarse + Vec3::new(-36.0, 0.0, 21.0), Vec3::ZERO);
    put(gen::regular_prism_mesh(3, 44.0, 26.0, false), coarse + Vec3::new(30.0, -24.0, 19.0), Vec3::ZERO);
    put(gen::cylinder_mesh(9.0, 9.0, 46.0, 12), coarse + Vec3::new(34.0, 26.0, 29.0), Vec3::ZERO);

    // -- standing on their own, turned every which way ---------------------
    let turned = 230.0;
    put(gen::box_mesh(56.0, 34.0, 22.0), Vec3::new(-260.0, turned, 40.0), Vec3::new(17.0, -43.0, 88.0));
    put(gen::cylinder_mesh(26.0, 26.0, 70.0, 32), Vec3::new(-130.0, turned, 30.0), Vec3::new(90.0, 0.0, 25.0));
    put(gen::regular_prism_mesh(6, 40.0, 50.0, false), Vec3::new(0.0, turned, 40.0), Vec3::new(-62.0, 30.0, 0.0));
    put(gen::cone_mesh(46.0, 18.0, 44.0, 24), Vec3::new(130.0, turned, 40.0), Vec3::new(140.0, 0.0, 0.0));
    put(gen::ellipsoid_mesh(44.0, 44.0, 44.0, 48), Vec3::new(260.0, turned, 40.0), Vec3::new(30.0, 30.0, 30.0));

    // -- the ones nothing here can rebuild, which must stay meshes ---------
    let awkward = 460.0;
    put(gen::torus_mesh(70.0, 20.0, 360.0, 64), Vec3::new(-260.0, awkward, 30.0), Vec3::ZERO);
    put(drilled(), Vec3::new(-130.0, awkward, 20.0), Vec3::ZERO);
    put(bracket(), Vec3::new(0.0, awkward, 20.0), Vec3::ZERO);
    put(gen::tube_mesh(56.0, 38.0, 44.0, 48), Vec3::new(130.0, awkward, 22.0), Vec3::ZERO);
    put(gen::capsule_mesh(28.0, 76.0, 32), Vec3::new(260.0, awkward, 38.0), Vec3::ZERO);
    put(gen::corner_box_mesh(60.0, 44.0, 30.0, 9.0, 6, false), Vec3::new(390.0, awkward, 15.0), Vec3::ZERO);
    put(
        gen::chamfered_box_mesh(60.0, 44.0, 30.0, 7.0, gen::ChamferEdges::All),
        Vec3::new(520.0, awkward, 15.0),
        Vec3::ZERO,
    );

    // -- a field of small cubes, for the cap -------------------------------
    for i in 0..60 {
        let (row, column) = (i / 12, i % 12);
        let at = Vec3::new(-260.0 + f64::from(column) * 26.0, 660.0 + f64::from(row) * 26.0, 5.0);
        put(gen::box_mesh(10.0, 10.0, 10.0), at, Vec3::ZERO);
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create the output directory");
    }
    let options = simple3d_export::Options {
        format: simple3d_export::Format::ThreeMf,
        scale: 1.0,
        unit: simple3d_export::Unit3mf::Millimeter,
        allow_invalid: false,
        // One object, so the import lands as one mesh node -- which is what
        // there is to reassemble. Written as separate bodies the importer would
        // hand back a group of meshes and the feature would have nothing to do.
        bodies: simple3d_export::BodyMode::One,
        compress: true,
    };
    let mut progress = |_: f32| true;
    if let Err(e) = simple3d_export::write(&path, &model, &options, &mut progress) {
        println!("FAILED {}: {e}", path.display());
        return;
    }
    println!("wrote {} -- {bodies} bodies, {} triangles", path.display(), model.triangle_count());
    report(&path, bodies);
}

/// Read the file back the way the application reads it, take it apart, and say
/// what came out.
///
/// The harness checks itself, because the thing that makes this file worth
/// having is not that it is complicated but that it is complicated in the ways
/// the recognition has to answer for -- and a body that quietly stopped being
/// recognised would otherwise only turn up as a disappointing afternoon with
/// the tool open.
fn report(path: &std::path::Path, bodies: usize) {
    let mut progress = |_: f32| true;
    let model = simple3d_import::read(path, &mut progress).expect("the file reads back");
    println!("read back as {} object(s), {} triangles", model.parts.len(), model.triangle_count());
    let plan = simple3d_geom::reassemble::Reassemble::default();
    let at = std::time::Instant::now();
    let found = simple3d_geom::reassemble::reassemble(&model.merged(), &plan);
    println!(
        "{:?}: {} bodies, {} recognised, {} groups (of {bodies} put in)",
        at.elapsed(),
        found.parts.len(),
        found.recognised(),
        found.groups.iter().filter(|group| group.len() > 1).count()
    );
    let mut counted: Vec<(&str, usize)> = Vec::new();
    for part in &found.parts {
        match counted.iter_mut().find(|(label, _)| *label == part.shape.label()) {
            Some((_, count)) => *count += 1,
            None => counted.push((part.shape.label(), 1)),
        }
    }
    counted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
    for (label, count) in counted {
        println!("  {count:>3} {label}");
    }
}

/// A plate with a hole bored through it: one closed body, and the case the
/// whole measurement is shaped around -- every corner of it sits on the surface
/// of its own bounding box, so anything that looked only at corners would call
/// it a solid box and lose the hole.
fn drilled() -> Mesh {
    let plate = gen::box_mesh(76.0, 76.0, 14.0);
    let drill = gen::cylinder_mesh(34.0, 34.0, 40.0, 48);
    simple3d_geom::evaluate_boolean(BooleanOp::Difference, &[plate, drill])
}

/// Two boxes welded into an L: one body whose surface turns a corner inwards,
/// which no single primitive describes.
fn bracket() -> Mesh {
    let upright = gen::box_mesh(22.0, 60.0, 70.0).translated(Vec3::new(-25.0, 0.0, 0.0));
    let foot = gen::box_mesh(72.0, 60.0, 20.0).translated(Vec3::new(0.0, 0.0, -25.0));
    simple3d_geom::evaluate_boolean(BooleanOp::Union, &[upright, foot])
}
