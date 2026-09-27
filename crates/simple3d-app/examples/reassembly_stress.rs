//! Writes the large 3MF used to stress the reassembly (issue 108): one object with about six
//! hundred bodies and a few hundred thousand triangles, exercising every setting the tool has.
//!
//! Unlike the small `reassembly_fixture`, this checks behaviour at scale and is too big to verify
//! by eye, so it reads the file back and prints a report. Bodies are appended, never unioned, as
//! in a printer file; only the two meant to be one shell go through the kernel.
//!
//! Contents:
//!
//! * **24 towers** of a plate and three parts at varied tessellations: 96 bodies in 24 groups.
//! * **Very fine tessellations** up to 256 segments: most triangles and the fit's cost; past
//!   128 sides the recognition must refuse and keep the mesh.
//! * **36 bodies at arbitrary angles**, from a fixed sequence, for the direction search.
//! * **23 unrebuildable shapes** that must stay meshes, including two shapes with one vertex out
//!   of place. The tetrahedron may come back as a three-sided cone, which rebuilds it exactly.
//! * **A ladder of cylinders dented 0.02mm to 1mm**, straddling the default 0.1mm tolerance.
//! * **Pairs at gaps up to 0.5mm and a chain of twelve**, around the 0.01mm contact slack;
//!   touching is transitive, so the chain is one group.
//! * **A 600mm slab and a 0.6mm cube**, since the tolerance is absolute.
//! * **400 bodies in a field of ramping sizes**, so the default cap of 200 cuts through the
//!   field: the largest bodies become objects and the rest one leftover mesh. A cap in index
//!   order would drop the towers instead.
//!
//! usage: reassembly_stress [out.3mf]

mod common;

use simple3d_geom::primitives as gen;
use simple3d_geom::{Mesh, Vec3};
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
    // Parts are sunk 1mm into their plate, and diameters vary per tower so no two bodies share a
    // vertex, which would weld them into one.
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
        // On its side, so one body per tower has a different axis.
        put(
            gen::cylinder_mesh(18.0 + wobble, 18.0 + wobble, 70.0, segments),
            base + Vec3::new(0.0, 36.0, 15.0),
            Vec3::new(90.0, 0.0, 0.0),
        );
    }

    // -- tessellated far finer than anything is modelled at ----------------
    let fine = 520.0;
    // 128 segments is the finest ring recognised, so this is the last cylinder and the next two
    // must stay meshes.
    put(gen::cylinder_mesh(70.0, 70.0, 80.0, 128), Vec3::new(-560.0, fine, 40.0), Vec3::ZERO);
    put(gen::cylinder_mesh(64.0, 64.0, 90.0, 192), Vec3::new(-420.0, fine, 45.0), Vec3::ZERO);
    put(gen::cylinder_mesh(58.0, 58.0, 74.0, 256), Vec3::new(-280.0, fine, 37.0), Vec3::ZERO);
    put(gen::ellipsoid_mesh(84.0, 84.0, 84.0, 96), Vec3::new(-140.0, fine, 42.0), Vec3::ZERO);
    // Squashed too: its ring vertices are not evenly spaced in direction.
    put(gen::ellipsoid_mesh(76.0, 52.0, 90.0, 128), Vec3::new(0.0, fine, 45.0), Vec3::ZERO);
    put(gen::cone_mesh(80.0, 26.0, 70.0, 160), Vec3::new(140.0, fine, 35.0), Vec3::ZERO);
    // The heaviest body, which must stay a mesh: a hundred thousand points measured against wrong fits.
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
            // The hardest fit: a triaxial ellipsoid at an arbitrary angle, which must stay a mesh rather
            // than become a roughly sized sphere.
            4 => put(gen::ellipsoid_mesh(size, size * 1.4, size * 0.8, 24 + i % 16), at, turn),
            // Slightly off an axis, so a fit quietly using the world axis would still measure plausibly.
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
    odd(common::drilled(80.0, 16.0, 38.0), &mut put);
    odd(common::bracket([24.0, 64.0, 74.0], [76.0, 64.0, 22.0], [-27.0, -26.0]), &mut put);
    // One vertex out of place, so the worst point rather than the average must decide.
    odd(dented(&gen::cylinder_mesh(60.0, 60.0, 70.0, 48), 1.4), &mut put);
    odd(dented(&gen::box_mesh(70.0, 50.0, 46.0), 1.1), &mut put);

    // -- a ladder of dents, straddling the tolerance -----------------------
    //
    // At the default 0.1mm the first three are cylinders and the last four meshes.
    for (i, by) in [0.02, 0.05, 0.09, 0.12, 0.2, 0.5, 1.0].iter().enumerate() {
        let at = Vec3::new(-560.0 + i as f64 * 110.0, 1300.0, 40.0);
        put(dented(&gen::cylinder_mesh(56.0, 56.0, 76.0, 48), *by), at, Vec3::ZERO);
    }

    // -- gaps either side of what counts as touching -----------------------
    //
    // With 0.01mm box slack, the first three pairs group and the last stays apart.
    for (i, gap) in [0.0, 0.002, 0.008, 0.5].iter().enumerate() {
        let at = Vec3::new(-560.0 + i as f64 * 150.0, 1450.0, 0.0);
        put(gen::box_mesh(60.0, 60.0, 30.0), at + Vec3::new(0.0, 0.0, 15.0), Vec3::ZERO);
        put(gen::cylinder_mesh(30.0, 30.0, 40.0, 32), at + Vec3::new(0.0, 0.0, 50.0 + gap), Vec3::ZERO);
    }
    // Boxes overlap though the solids do not touch (an L); grouped, since grouping works on boxes.
    put(gen::box_mesh(24.0, 60.0, 90.0), Vec3::new(40.0, 1450.0, 45.0), Vec3::ZERO);
    put(gen::box_mesh(90.0, 60.0, 24.0), Vec3::new(120.0, 1450.0, 12.0), Vec3::ZERO);
    // A chain of twelve, each link touching only the next: one group. Spaced 0.004mm (under the
    // slack) rather than flush, since flush links would share vertices and be one body.
    for i in 0..12u32 {
        let at = Vec3::new(260.0 + f64::from(i) * 30.004, 1450.0, 15.0);
        put(gen::box_mesh(30.0, 40.0, 30.0), at, Vec3::ZERO);
    }

    // -- the whole range of sizes, in one place ----------------------------
    put(gen::box_mesh(600.0, 120.0, 20.0), Vec3::new(-260.0, 1620.0, 10.0), Vec3::ZERO);
    put(gen::ellipsoid_mesh(300.0, 300.0, 300.0, 64), Vec3::new(240.0, 1700.0, 150.0), Vec3::ZERO);
    // Clear of the slab so it tests the tolerance, not the grouping.
    put(gen::box_mesh(0.6, 0.6, 0.6), Vec3::new(-540.0, 1520.0, 0.3), Vec3::ZERO);
    put(gen::cylinder_mesh(1.5, 1.5, 6.0, 16), Vec3::new(-530.0, 1520.0, 3.0), Vec3::ZERO);
    put(gen::cylinder_mesh(4.0, 4.0, 0.4, 24), Vec3::new(-518.0, 1520.0, 0.2), Vec3::ZERO);

    // -- four hundred in a field, for the cap ------------------------------
    //
    // Sized so the default cap of 200 cuts through the field.
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

    if let Some(read) = common::write_and_read_back(&path, &model, bodies) {
        report(&read, bodies);
    }
}

/// Reassemble what was read back at the default settings and at a different cap and tolerance,
/// printing what came out.
fn report(read: &simple3d_import::Model, bodies: usize) {
    let mesh = read.merged();
    let default = simple3d_geom::reassemble::Reassemble::default();
    run("default", &mesh, &default, bodies);
    run("no cap", &mesh, &simple3d_geom::reassemble::Reassemble { max_objects: 4000, ..default }, bodies);
    run("tolerance 1mm", &mesh, &simple3d_geom::reassemble::Reassemble { tolerance: 1.0, ..default }, bodies);
    run("tolerance 0.01mm", &mesh, &simple3d_geom::reassemble::Reassemble { tolerance: 0.01, ..default }, bodies);
    run("bodies only", &mesh, &simple3d_geom::reassemble::Reassemble { recognise: false, ..default }, bodies);
}

/// One reassembly run, printed as a line and a tally of body outcomes.
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
    let tally: Vec<String> = common::tally(&found).iter().map(|(label, count)| format!("{count} {label}")).collect();
    println!("  {}", tally.join(", "));
}

/// The same body with one corner pushed `by` millimetres along X. Every copy moves, so the
/// surface stays closed.
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

/// A linear congruential generator, so the file is identical on every machine.
struct Rng(u64);

impl Rng {
    fn seeded(seed: u64) -> Rng {
        Rng(seed)
    }

    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    /// An angle in degrees, anywhere in the turn.
    fn angle(&mut self) -> f64 {
        self.next() * 360.0
    }

    /// A number from zero up to `most`.
    fn upto(&mut self, most: f64) -> f64 {
        self.next() * most
    }
}
