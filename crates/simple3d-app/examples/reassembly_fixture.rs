//! Writes the 3MF used to try the reassembly (issue 108): one object of eighty-odd separate
//! bodies covering every branch of the recognition.
//!
//! Bodies are appended, never unioned, as in a printer file; a union would weld touching bodies
//! into one. Shapes meant to be one shell (drilled plate, L-bracket) go through the kernel.
//!
//! * **Two assemblies on plates**: each should become one group with every part named, and the
//!   two should stay apart.
//! * **Shapes turned every which way**: the fits must find the axis.
//! * **Unrebuildable shapes** (torus, drilled plate, L-bracket, tube, capsule, rounded box): must
//!   stay meshes; recognising one is worse than missing a cylinder.
//! * **Sixty small cubes**, for the cap: lowering it sweeps them into the leftover mesh.
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
        // Sunk 2 mm into the plate, as parts meant to be in contact are modelled.
        put(gen::cylinder_mesh(14.0, 14.0, 40.0, 32), plate + Vec3::new(x, y, 26.0), Vec3::ZERO);
    }
    put(gen::regular_prism_mesh(6, 44.0, 22.0, false), plate + Vec3::new(0.0, 0.0, 17.0), Vec3::ZERO);

    // -- assembly two: a stack that comes to a point -----------------------
    let stack = Vec3::new(-40.0, 0.0, 0.0);
    put(gen::cylinder_mesh(90.0, 90.0, 12.0, 64), stack + Vec3::new(0.0, 0.0, 6.0), Vec3::ZERO);
    put(gen::cone_mesh(78.0, 30.0, 34.0, 64), stack + Vec3::new(0.0, 0.0, 27.0), Vec3::ZERO);
    // Wider than the frustum and sunk into it: matching its rim would share vertices and weld them.
    put(gen::cone_mesh(34.0, 0.0, 28.0, 48), stack + Vec3::new(0.0, 0.0, 56.0), Vec3::ZERO);
    put(gen::ellipsoid_mesh(24.0, 24.0, 24.0, 32), stack + Vec3::new(0.0, 0.0, 78.0), Vec3::ZERO);

    // -- assembly three: the coarse tessellations --------------------------
    let coarse = Vec3::new(160.0, 0.0, 0.0);
    put(gen::box_mesh(120.0, 90.0, 8.0), coarse + Vec3::new(0.0, 0.0, 4.0), Vec3::ZERO);
    // Eight segments: few enough that a prism is what was meant.
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
        // One object, so the import is one mesh node to reassemble.
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

/// Read the file back as the application does, reassemble it, and report what came out.
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

/// A plate with a hole: every corner lies on its bounding box, so a corners-only measure would
/// call it a solid box.
fn drilled() -> Mesh {
    let plate = gen::box_mesh(76.0, 76.0, 14.0);
    let drill = gen::cylinder_mesh(34.0, 34.0, 40.0, 48);
    simple3d_geom::evaluate_boolean(BooleanOp::Difference, &[plate, drill])
}

/// Two boxes welded into an L, which no single primitive describes.
fn bracket() -> Mesh {
    let upright = gen::box_mesh(22.0, 60.0, 70.0).translated(Vec3::new(-25.0, 0.0, 0.0));
    let foot = gen::box_mesh(72.0, 60.0, 20.0).translated(Vec3::new(0.0, 0.0, -25.0));
    simple3d_geom::evaluate_boolean(BooleanOp::Union, &[upright, foot])
}
