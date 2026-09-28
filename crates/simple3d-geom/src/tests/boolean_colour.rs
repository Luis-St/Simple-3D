//! The colour of what a boolean exposes (issue 114).

use crate::mesh::{colour_tag, Mesh};
use crate::primitives;
use crate::vec3::Vec3;
use crate::{csg_bsp, evaluate_boolean, BooleanOp};

fn tagged(mut mesh: Mesh, tag: u32) -> Mesh {
    mesh.set_tag(tag);
    mesh
}

#[test]
fn the_walls_a_difference_opens_take_the_colour_of_what_it_cuts() {
    let red = colour_tag([200, 30, 30]);
    let blue = colour_tag([30, 30, 200]);
    let plate = tagged(primitives::box_mesh(40.0, 40.0, 10.0), red);
    let drill = tagged(primitives::cylinder_mesh(10.0, 10.0, 30.0, 24).translated(Vec3::new(0.0, 0.0, -10.0)), blue);
    let drilled = csg_bsp::subtract(&plate, &drill);
    assert!(drilled.triangle_count() > plate.triangle_count(), "nothing was drilled");
    assert!(
        (0..drilled.triangle_count()).all(|t| drilled.tag(t) == red),
        "the bore kept the drill's colour instead of the plate's"
    );

    // An unpainted body stays unpainted inside, whatever colour the cutter has.
    let plain = csg_bsp::subtract(&tagged(primitives::box_mesh(40.0, 40.0, 10.0), 0), &drill);
    assert!((0..plain.triangle_count()).all(|t| plain.tag(t) == 0), "an unpainted plate's bore was painted");
}

#[test]
fn a_cut_through_a_two_coloured_body_takes_the_colour_of_the_part_it_opens() {
    // A red and a green slab side by side, drilled through the red one only.
    let red = colour_tag([200, 30, 30]);
    let green = colour_tag([30, 200, 30]);
    let mut slabs = tagged(primitives::box_mesh(40.0, 40.0, 10.0).translated(Vec3::new(-20.0, 0.0, 0.0)), red);
    slabs.append(&tagged(primitives::box_mesh(40.0, 40.0, 10.0).translated(Vec3::new(40.0, 0.0, 0.0)), green));
    let drill = primitives::cylinder_mesh(10.0, 10.0, 30.0, 24).translated(Vec3::new(-20.0, 0.0, -10.0));
    let drilled = evaluate_boolean(BooleanOp::Difference, &[slabs, drill]);
    let walls: Vec<u32> = (0..drilled.triangle_count())
        .filter(|&t| {
            let [a, b, c] = drilled.corners(drilled.indices[t]);
            let centre = (a + b + c) * (1.0 / 3.0);
            ((centre.x + 20.0).powi(2) + centre.y.powi(2)).sqrt() < 5.5 && centre.z > 0.1 && centre.z < 9.9
        })
        .map(|t| drilled.tag(t))
        .collect();
    assert!(!walls.is_empty(), "the bore has no walls");
    assert!(walls.iter().all(|&tag| tag == red), "the red slab's bore is not red: {walls:?}");
}
