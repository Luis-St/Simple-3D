//! Writes the big 3MF the reassembly is put under load with (issue 108): one
//! object holding some six hundred separate bodies and a few hundred thousand
//! triangles, built so that every number the tool exposes has something in the
//! file that turns on it.
//!
//! The curated fixture next door, `reassembly_fixture`, is the one that asks
//! whether each branch of the recognition answers correctly, and it is small
//! enough to read off by eye. This one asks the other question -- whether the
//! answers survive scale, tessellation, odd angles and bodies that sit a
//! hundredth of a millimetre apart -- and is deliberately too big to check by
//! counting. What makes it usable anyway is the report at the end: the harness
//! reads the file back, takes it apart and prints what came out, so a body
//! that quietly stopped being recognised shows up as a number rather than as a
//! disappointing afternoon with the tool open.
//!
//! As in the small fixture the bodies are *appended*, never unioned: a printer
//! file is a bag of surfaces and says nothing about which of them are one
//! solid, and putting them through the boolean kernel first would weld them
//! into a different model. The two bodies that are *meant* to be one shell go
//! through the kernel on purpose.
//!
//! What is in it, and what each part of it loads:
//!
//! * **A rack of twenty-four towers** -- each a plate with three parts standing
//!   on it, at a different tessellation and a different set of shapes per
//!   tower. Ninety-six bodies in twenty-four groups: the grouping is a pass
//!   over every pair of parts, so this is where that shows, and the towers
//!   stand far enough apart that two of them running together would be plain.
//! * **A row tessellated far finer than anything is modelled at** -- up to two
//!   hundred and fifty-six segments. Most of the file's triangles are in these
//!   seven bodies, and they are the ones that say what the fit costs when the
//!   measurement is over a hundred thousand points. The recognition stops
//!   reading a ring at a hundred and twenty-eight sides, so the finer half of
//!   the row is there to show that the limit is a *refusal* -- they come back
//!   as the meshes they are, not as some cylinder of the wrong radius.
//! * **Thirty-six bodies at angles off every axis** -- the fit searches a dozen
//!   directions found from the body itself, so a body turned to no particular
//!   angle is what that search is for. The angles are drawn from a fixed
//!   sequence, so the file is the same file every time it is written.
//! * **Twenty-three shapes nothing here can rebuild** -- the sectors, the tube,
//!   the torus, the platonic solids, the wedge, a drilled plate, an L-bracket,
//!   and two bodies that are a cylinder and a box except for one vertex pushed
//!   out of place. All of these must come back as meshes; recognising one is a
//!   worse failure than missing a cylinder, because it silently replaces the
//!   model with a different solid. The one exception is the tetrahedron, which
//!   is not an exception at all: a regular tetrahedron *is* a three-sided cone
//!   closed to a point, so coming back as one is the right answer and rebuilds
//!   the body exactly.
//! * **A ladder of cylinders dented by 0.02mm up to 1mm** -- straddling the
//!   default tolerance of a tenth of a millimetre. Nothing else in either file
//!   makes the tolerance slider do anything visible; here, dragging it walks
//!   the recognised count up the ladder one rung at a time.
//! * **Pairs at gaps from nothing to half a millimetre, and a chain of twelve**
//!   -- the grouping calls bodies touching within a hundredth of a millimetre,
//!   so the pairs either side of that are the test, and the chain is there
//!   because touching is transitive: twelve bodies, each meeting only its
//!   neighbour, are one group.
//! * **Bodies six hundred millimetres and six tenths of one, side by side** --
//!   the tolerance is an absolute distance, so the same tenth of a millimetre
//!   is nothing on the slab and is the whole of the tiny cube.
//! * **Four hundred bodies in a field, in sizes that ramp** -- twice the
//!   default cap on their own, and two thirds of the file. Past the cap the
//!   biggest bodies are the ones that become objects, and the field is sized
//!   so that the cut falls inside it: at the default cap of two hundred the
//!   field's largest few cubes become objects, everything smaller than them --
//!   the rest of the field, and the grain of sand and the pin from the row
//!   above -- goes into the one leftover mesh, and every real part of the file
//!   is kept. A cap applied in index order instead would take the field's
//!   first two hundred and leave the towers out, which is the failure the
//!   ordering is there to prevent.
//!
//! usage: reassembly_stress [out.3mf]

use simple3d_geom::primitives as gen;
use simple3d_geom::{BooleanOp, Mesh, Vec3};
use std::path::PathBuf;

fn main() {
    let path = std::env::args().nth(1).map_or_else(|| PathBuf::from("exports/reassembly-stress.3mf"), PathBuf::from);
    let mut model = Mesh::new();
    let mut bodies = 0;
    let mut put = |mesh: Mesh, at: Vec3, turn: Vec3| {
        model.append(&mesh.transformed(at, turn));
        bodies += 1;
    };

    // -- a rack of towers, each tessellated differently -------------------
    //
    // The parts are sunk a millimetre into the plate under them, the way a
    // part that is meant to be in contact is modelled, and each tower's
    // diameters are shifted a little from its neighbour's so that no two
    // bodies in the file can put a vertex in the same place -- a shared vertex
    // is what "one body" means here, and two towers welded into one would be a
    // fault in the fixture rather than in the tool.
    for tower in 0..24u32 {
        let (row, column) = (tower / 8, tower % 8);
        let base = Vec3::new(-560.0 + f64::from(column) * 160.0, f64::from(row) * 160.0, 0.0);
        let segments = [8, 12, 16, 24, 32, 48, 64, 96][column as usize];
        let wobble = f64::from(tower) * 0.37;
        put(gen::box_mesh(120.0 + wobble, 120.0 + wobble, 7.0), base + Vec3::new(0.0, 0.0, 3.5), Vec3::ZERO);
        put(
            gen::cylinder_mesh(30.0 + wobble, 30.0 + wobble, 36.0, segments),
            base + Vec3::new(-32.0, -30.0, 24.0),
            Vec3::ZERO,
        );
        match row {
            0 => put(
                gen::regular_prism_mesh(3 + column % 6, 34.0 + wobble, 30.0, false),
                base + Vec3::new(34.0, -28.0, 21.0),
                Vec3::ZERO,
            ),
            1 => put(
                gen::cone_mesh(38.0 + wobble, 14.0 + wobble, 32.0, segments),
                base + Vec3::new(34.0, -28.0, 22.0),
                Vec3::ZERO,
            ),
            _ => put(
                gen::ellipsoid_mesh(36.0 + wobble, 36.0 + wobble, 36.0, segments),
                base + Vec3::new(34.0, -28.0, 24.0),
                Vec3::ZERO,
            ),
        }
        // Lying on its side, so the tower holds one body whose axis is not the
        // one every other body in it stands on.
        put(
            gen::cylinder_mesh(18.0 + wobble, 18.0 + wobble, 70.0, segments),
            base + Vec3::new(0.0, 36.0, 15.0),
            Vec3::new(90.0, 0.0, 0.0),
        );
    }

    // -- tessellated far finer than anything is modelled at ----------------
    let fine = 520.0;
    // A hundred and twenty-eight segments is the finest ring the recognition
    // reads, so this one is the last that comes back as a cylinder and the two
    // after it are the first that do not.
    put(gen::cylinder_mesh(70.0, 70.0, 80.0, 128), Vec3::new(-560.0, fine, 40.0), Vec3::ZERO);
    put(gen::cylinder_mesh(64.0, 64.0, 90.0, 192), Vec3::new(-420.0, fine, 45.0), Vec3::ZERO);
    put(gen::cylinder_mesh(58.0, 58.0, 74.0, 256), Vec3::new(-280.0, fine, 37.0), Vec3::ZERO);
    put(gen::ellipsoid_mesh(84.0, 84.0, 84.0, 96), Vec3::new(-140.0, fine, 42.0), Vec3::ZERO);
    // Squashed as well as finely tessellated: the vertices of an ellipsoid
    // whose two equatorial diameters differ are not evenly spaced around it,
    // and the fit reads a ring by the directions its vertices stand in.
    put(gen::ellipsoid_mesh(76.0, 52.0, 90.0, 128), Vec3::new(0.0, fine, 45.0), Vec3::ZERO);
    put(gen::cone_mesh(80.0, 26.0, 70.0, 160), Vec3::new(140.0, fine, 35.0), Vec3::ZERO);
    // The heaviest body in the file, and one that must stay a mesh: the cost of
    // a hundred thousand points being measured against a dozen fits, all of
    // which are wrong.
    put(gen::torus_mesh(90.0, 26.0, 360.0, 256), Vec3::new(300.0, fine, 40.0), Vec3::ZERO);

    // -- turned to no particular angle -------------------------------------
    let mut rng = Rng::seeded(0x5eed_1108);
    for i in 0..36u32 {
        let (row, column) = (i / 12, i % 12);
        let at = Vec3::new(-560.0 + f64::from(column) * 100.0, 660.0 + f64::from(row) * 100.0, 60.0);
        let turn = Vec3::new(rng.angle(), rng.angle(), rng.angle());
        let size = 26.0 + rng.upto(14.0);
        match i % 6 {
            0 => put(gen::box_mesh(size * 1.7, size, size * 0.6), at, turn),
            1 => put(gen::cylinder_mesh(size, size, size * 2.2, 16 + i % 24), at, turn),
            2 => put(gen::regular_prism_mesh(3 + i % 7, size * 1.6, size, false), at, turn),
            3 => put(gen::cone_mesh(size * 1.8, size * 0.4, size * 1.5, 12 + i % 32), at, turn),
            // The hardest fit in the file, and the one that mostly fails: an
            // ellipsoid with three different diameters, turned to an angle
            // that is on nothing. It is here to be sure that what a fit
            // cannot find comes back as the body's own triangles rather than
            // as a sphere of roughly the right size.
            4 => put(gen::ellipsoid_mesh(size, size * 1.4, size * 0.8, 24 + i % 16), at, turn),
            // A hair off an axis rather than nowhere near one: the search has
            // to find the body's own direction, and an axis it nearly agrees
            // with is where a fit that quietly used the world's instead would
            // still measure close enough to be believed.
            _ => put(gen::cylinder_mesh(size, size, size * 2.0, 32), at, Vec3::new(0.7, -0.4, rng.angle())),
        }
    }

    // -- the ones nothing here can rebuild, which must stay meshes ---------
    let awkward = 1000.0;
    let mut spot = 0;
    let mut odd = |mesh: Mesh, put: &mut dyn FnMut(Mesh, Vec3, Vec3)| {
        let (row, column) = (spot / 10, spot % 10);
        spot += 1;
        put(mesh, Vec3::new(-560.0 + f64::from(column) * 120.0, awkward + f64::from(row) * 120.0, 50.0), Vec3::ZERO);
    };
    odd(gen::tube_mesh(70.0, 44.0, 60.0, 64), &mut put);
    odd(gen::ring_mesh(80.0, 30.0, 14.0, 48), &mut put);
    odd(gen::torus_mesh(64.0, 22.0, 220.0, 48), &mut put);
    odd(gen::capsule_mesh(34.0, 90.0, 32), &mut put);
    odd(gen::cylinder_sector_mesh(80.0, 80.0, 50.0, 48, 110.0), &mut put);
    odd(gen::cone_sector_mesh(80.0, 30.0, 60.0, 48, 250.0), &mut put);
    odd(gen::tube_sector_mesh(80.0, 50.0, 40.0, 48, 90.0), &mut put);
    odd(gen::spherical_cap_mesh(90.0, 30.0, 48), &mut put);
    odd(gen::slot_mesh(90.0, 34.0, 26.0, 24), &mut put);
    odd(gen::rounded_plate_mesh(80.0, 60.0, 16.0, 12.0, 12), &mut put);
    odd(gen::rounded_box_mesh(70.0, 50.0, 40.0, 10.0, 10), &mut put);
    odd(gen::chamfered_box_mesh(70.0, 50.0, 40.0, 8.0, gen::ChamferEdges::All), &mut put);
    odd(gen::corner_box_mesh(70.0, 50.0, 40.0, 11.0, 8, false), &mut put);
    odd(gen::wedge_mesh(80.0, 50.0, 46.0, 22.0), &mut put);
    odd(gen::pyramid_mesh(70.0, 50.0, 18.0, 34.0, 44.0), &mut put);
    odd(gen::tetrahedron_mesh(70.0, false), &mut put);
    odd(gen::octahedron_mesh(70.0, false), &mut put);
    odd(gen::dodecahedron_mesh(64.0, false), &mut put);
    odd(gen::icosahedron_mesh(64.0, false), &mut put);
    odd(drilled(), &mut put);
    odd(bracket(), &mut put);
    // A cylinder and a box with one vertex pushed well out of place. Every
    // other measurement on them is exact, so they are the case where the fit
    // has to be decided by the worst point on the body rather than by the
    // average of it.
    odd(dented(&gen::cylinder_mesh(60.0, 60.0, 70.0, 48), 1.4), &mut put);
    odd(dented(&gen::box_mesh(70.0, 50.0, 46.0), 1.1), &mut put);

    // -- a ladder of dents, straddling the tolerance -----------------------
    //
    // Each of these is a cylinder to within the millimetres named, and nothing
    // else: with the default tenth of a millimetre the first three are
    // cylinders and the last four are meshes, and moving the slider walks the
    // boundary along the row.
    for (i, by) in [0.02, 0.05, 0.09, 0.12, 0.2, 0.5, 1.0].iter().enumerate() {
        let at = Vec3::new(-560.0 + i as f64 * 110.0, 1300.0, 40.0);
        put(dented(&gen::cylinder_mesh(56.0, 56.0, 76.0, 48), *by), at, Vec3::ZERO);
    }

    // -- gaps either side of what counts as touching -----------------------
    //
    // The grouping reads contact off the bodies' boxes with a hundredth of a
    // millimetre of slack, so the first three pairs should each come back as a
    // group of two and the last should be two objects standing apart.
    for (i, gap) in [0.0, 0.002, 0.008, 0.5].iter().enumerate() {
        let at = Vec3::new(-560.0 + i as f64 * 150.0, 1450.0, 0.0);
        put(gen::box_mesh(60.0, 60.0, 30.0), at + Vec3::new(0.0, 0.0, 15.0), Vec3::ZERO);
        put(gen::cylinder_mesh(30.0, 30.0, 40.0, 32), at + Vec3::new(0.0, 0.0, 50.0 + gap), Vec3::ZERO);
    }
    // Two bodies whose boxes overlap although the solids never touch: an
    // upright and a foot set apart in an L. They are grouped, because the
    // grouping is a question about intent asked at the scale of a box, and the
    // file should say so plainly rather than leave it looking like a bug.
    put(gen::box_mesh(24.0, 60.0, 90.0), Vec3::new(40.0, 1450.0, 45.0), Vec3::ZERO);
    put(gen::box_mesh(90.0, 60.0, 24.0), Vec3::new(120.0, 1450.0, 12.0), Vec3::ZERO);
    // A chain: each link meets only the next, and all twelve are one group.
    // Set four thousandths of a millimetre apart rather than flush, which is
    // under the slack and so still touching -- flush, each link would put its
    // corners exactly where its neighbour's are, and twelve bodies sharing
    // their vertices are one body.
    for i in 0..12u32 {
        let at = Vec3::new(260.0 + f64::from(i) * 30.004, 1450.0, 15.0);
        put(gen::box_mesh(30.0, 40.0, 30.0), at, Vec3::ZERO);
    }

    // -- the whole range of sizes, in one place ----------------------------
    put(gen::box_mesh(600.0, 120.0, 20.0), Vec3::new(-260.0, 1620.0, 10.0), Vec3::ZERO);
    put(gen::ellipsoid_mesh(300.0, 300.0, 300.0, 64), Vec3::new(240.0, 1700.0, 150.0), Vec3::ZERO);
    // Standing clear of the slab rather than beside it: a body the size of a
    // grain of sand whose box touched the slab's would be grouped with it, and
    // what these are here for is the tolerance, not the grouping.
    put(gen::box_mesh(0.6, 0.6, 0.6), Vec3::new(-540.0, 1520.0, 0.3), Vec3::ZERO);
    put(gen::cylinder_mesh(1.5, 1.5, 6.0, 16), Vec3::new(-530.0, 1520.0, 3.0), Vec3::ZERO);
    put(gen::cylinder_mesh(4.0, 4.0, 0.4, 24), Vec3::new(-518.0, 1520.0, 0.2), Vec3::ZERO);

    // -- four hundred in a field, for the cap ------------------------------
    //
    // Sized so that the field itself is cut in half by the default cap of two
    // hundred: everything above it is bigger than anything here, the smallest
    // of these are the last thing to become an object, and what is left goes
    // into the one leftover mesh.
    for i in 0..400u32 {
        let (row, column) = (i / 20, i % 20);
        let at = Vec3::new(-560.0 + f64::from(column) * 34.0, 1900.0 + f64::from(row) * 34.0, 0.0);
        let size = 3.0 + f64::from(i) * 0.05;
        if i % 3 == 0 {
            put(gen::cylinder_mesh(size, size, size, 8), at + Vec3::new(0.0, 0.0, size / 2.0), Vec3::ZERO);
        } else {
            put(gen::box_mesh(size, size, size), at + Vec3::new(0.0, 0.0, size / 2.0), Vec3::ZERO);
        }
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

/// Read the file back the way the application reads it, take it apart at the
/// default settings and at a couple of others, and say what came out.
///
/// The second and third runs are the point of a file this size: the cap and
/// the tolerance are the two settings that do nothing on a small model, and a
/// run at each of them is how the rows built for them are read.
fn report(path: &std::path::Path, bodies: usize) {
    let mut progress = |_: f32| true;
    let model = simple3d_import::read(path, &mut progress).expect("the file reads back");
    println!("read back as {} object(s), {} triangles", model.parts.len(), model.triangle_count());
    let mesh = model.merged();
    let default = simple3d_geom::reassemble::Reassemble::default();
    run("default", &mesh, &default, bodies);
    run("no cap", &mesh, &simple3d_geom::reassemble::Reassemble { max_objects: 4000, ..default }, bodies);
    run("tolerance 1mm", &mesh, &simple3d_geom::reassemble::Reassemble { tolerance: 1.0, ..default }, bodies);
    run("tolerance 0.01mm", &mesh, &simple3d_geom::reassemble::Reassemble { tolerance: 0.01, ..default }, bodies);
    run("bodies only", &mesh, &simple3d_geom::reassemble::Reassemble { recognise: false, ..default }, bodies);
}

/// One run of the reassembly over the whole mesh, printed as a line and a
/// tally of what each body turned out to be.
fn run(what: &str, mesh: &Mesh, plan: &simple3d_geom::reassemble::Reassemble, bodies: usize) {
    let at = std::time::Instant::now();
    let found = simple3d_geom::reassemble::reassemble(mesh, plan);
    println!(
        "{what}: {:?} -- {} objects, {} recognised, {} groups, {} bodies left over (of {bodies} put in)",
        at.elapsed(),
        found.objects(),
        found.recognised(),
        found.groups.iter().filter(|group| group.len() > 1).count(),
        found.rest_bodies
    );
    let mut counted: Vec<(&str, usize)> = Vec::new();
    for part in &found.parts {
        match counted.iter_mut().find(|(label, _)| *label == part.shape.label()) {
            Some((_, count)) => *count += 1,
            None => counted.push((part.shape.label(), 1)),
        }
    }
    counted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));
    let tally: Vec<String> = counted.iter().map(|(label, count)| format!("{count} {label}")).collect();
    println!("  {}", tally.join(", "));
}

/// A plate with a hole bored through it: one closed body, and the case the
/// whole measurement is shaped around -- every corner of it sits on the surface
/// of its own bounding box, so anything that looked only at corners would call
/// it a solid box and lose the hole.
fn drilled() -> Mesh {
    let plate = gen::box_mesh(80.0, 80.0, 16.0);
    let drill = gen::cylinder_mesh(38.0, 38.0, 40.0, 48);
    simple3d_geom::evaluate_boolean(BooleanOp::Difference, &[plate, drill])
}

/// Two boxes welded into an L: one body whose surface turns a corner inwards,
/// which no single primitive describes.
fn bracket() -> Mesh {
    let upright = gen::box_mesh(24.0, 64.0, 74.0).translated(Vec3::new(-27.0, 0.0, 0.0));
    let foot = gen::box_mesh(76.0, 64.0, 22.0).translated(Vec3::new(0.0, 0.0, -26.0));
    simple3d_geom::evaluate_boolean(BooleanOp::Union, &[upright, foot])
}

/// The same body with one corner pushed `by` millimetres further out along X.
///
/// Every copy of that corner moves, so the surface stays closed and the file
/// stays valid: what changes is one point of one body, which is exactly what
/// the recognition is supposed to notice -- a shape is that shape when
/// *nothing* on it is further than the tolerance away, and a body that is
/// perfect but for one vertex is the cheapest way to say so.
fn dented(mesh: &Mesh, by: f64) -> Mesh {
    let mut out = mesh.clone();
    let Some(&target) = out.positions.iter().max_by(|a, b| a.x.total_cmp(&b.x).then(a.z.total_cmp(&b.z))) else {
        return out;
    };
    for p in &mut out.positions {
        if (*p - target).length() < 1e-9 {
            p.x += by;
        }
    }
    out
}

/// The sequence the angles and sizes are drawn from: a plain linear
/// congruential generator, written out here so that the file is the same file
/// on every machine that writes it and a body's angle can be looked up again.
struct Rng(u64);

impl Rng {
    fn seeded(seed: u64) -> Rng {
        Rng(seed)
    }

    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    /// An angle anywhere in the turn, in degrees.
    fn angle(&mut self) -> f64 {
        self.next() * 360.0
    }

    /// A number from nothing up to `most`.
    fn upto(&mut self, most: f64) -> f64 {
        self.next() * most
    }
}
