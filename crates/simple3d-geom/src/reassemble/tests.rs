//! What a mesh is recognised as, and what it is not.

use super::*;
use crate::primitives as gen;

fn plan() -> Reassemble {
    Reassemble::default()
}

/// Two meshes apart as one bag of triangles, like an imported assembly.
fn beside(a: &Mesh, b: &Mesh, apart: Vec3) -> Mesh {
    let mut out = a.clone();
    out.append(&b.translated(apart));
    out
}

fn shapes(assembly: &Assembly) -> Vec<Shape> {
    assembly.parts.iter().map(|part| part.shape).collect()
}

/// How far the parts are from the input mesh, measured independently of the run's report.
fn worst(before: &Mesh, assembly: &Assembly) -> f64 {
    let mut rebuilt = Mesh::new();
    for part in &assembly.parts {
        rebuilt.append(&part.placed());
    }
    if let Some(rest) = &assembly.rest {
        rebuilt.append(rest);
    }
    crate::simplify::measure::furthest_from(&before.weld().positions, &rebuilt)
}

#[test]
fn a_box_comes_back_as_a_box() {
    let before = gen::box_mesh(40.0, 30.0, 10.0);
    let found = reassemble(&before, &plan());
    assert_eq!(shapes(&found), vec![Shape::Box { width: 40.0, depth: 30.0, height: 10.0 }]);
    assert!(found.parts[0].deviation <= 1e-9, "a box is exactly a box, not {}", found.parts[0].deviation);
}

#[test]
fn a_cylinder_keeps_its_segments() {
    // The segment count must survive, or the rebuilt solid's corner width would differ.
    let before = gen::cylinder_mesh(20.0, 20.0, 50.0, 12);
    let found = reassemble(&before, &plan());
    assert_eq!(shapes(&found).len(), 1);
    match found.parts[0].shape {
        // Twelve sides reads as a prism, the same solid; what matters is the count.
        Shape::Prism { sides, diameter, height } => {
            assert_eq!(sides, 12);
            assert!((diameter - 20.0).abs() < 1e-6, "diameter {diameter}");
            assert!((height - 50.0).abs() < 1e-6, "height {height}");
        }
        other => panic!("a twelve-sided extrusion came back as {other:?}"),
    }
    assert!(worst(&before, &found) < 1e-6);
}

#[test]
fn a_finely_tessellated_cylinder_is_a_cylinder() {
    let before = gen::cylinder_mesh(12.0, 12.0, 40.0, 32);
    let found = reassemble(&before, &plan());
    assert_eq!(shapes(&found), vec![Shape::Cylinder { diameter: 12.0, height: 40.0, segments: 32 }]);
    assert!(worst(&before, &found) < 1e-6);
}

#[test]
fn a_sphere_comes_back_as_a_sphere() {
    let before = gen::ellipsoid_mesh(24.0, 24.0, 24.0, 24);
    let found = reassemble(&before, &plan());
    match found.parts[0].shape {
        Shape::Sphere { diameter_x, diameter_z, segments, .. } => {
            assert_eq!(segments, 24);
            assert!((diameter_x - 24.0).abs() < 1e-6, "diameter {diameter_x}");
            assert!((diameter_z - 24.0).abs() < 1e-6, "diameter {diameter_z}");
        }
        other => panic!("a sphere came back as {other:?}"),
    }
    assert!(worst(&before, &found) < 1e-6);
}

#[test]
fn a_cone_comes_back_as_a_cone() {
    let before = gen::cone_mesh(30.0, 10.0, 25.0, 24);
    let found = reassemble(&before, &plan());
    match found.parts[0].shape {
        Shape::Cone { bottom_diameter, top_diameter, height, segments } => {
            assert_eq!(segments, 24);
            assert!((height - 25.0).abs() < 1e-6, "height {height}");
            // Which end is the bottom depends on the guessed axis direction.
            let (small, big) = (bottom_diameter.min(top_diameter), bottom_diameter.max(top_diameter));
            assert!((small - 10.0).abs() < 1e-6 && (big - 30.0).abs() < 1e-6, "{small} and {big}");
        }
        other => panic!("a cone came back as {other:?}"),
    }
    assert!(worst(&before, &found) < 1e-6);
}

#[test]
fn a_turned_box_comes_back_turned() {
    let turn = Vec3::new(20.0, -35.0, 50.0);
    let before = gen::box_mesh(40.0, 30.0, 10.0).transformed(Vec3::new(5.0, -7.0, 11.0), turn);
    let found = reassemble(&before, &plan());
    assert!(matches!(found.parts[0].shape, Shape::Box { .. }), "{:?}", found.parts[0].shape);
    // The angles may differ (a box has several equivalent orientations); the rebuilt surface must match.
    assert!(worst(&before, &found) < 1e-6, "off by {}", worst(&before, &found));
    assert!((found.parts[0].centre - Vec3::new(5.0, -7.0, 11.0)).length() < 1e-6);
}

#[test]
fn separate_bodies_become_separate_parts() {
    let before =
        beside(&gen::box_mesh(10.0, 10.0, 10.0), &gen::ellipsoid_mesh(8.0, 8.0, 8.0, 16), Vec3::new(50.0, 0.0, 0.0));
    let found = reassemble(&before, &plan());
    assert_eq!(found.parts.len(), 2);
    assert!(found.parts.iter().any(|part| matches!(part.shape, Shape::Box { .. })));
    assert!(found.parts.iter().any(|part| matches!(part.shape, Shape::Sphere { .. })));
    assert_eq!(found.groups.len(), 2);
}

#[test]
fn bodies_that_touch_are_grouped() {
    let plate = gen::box_mesh(40.0, 40.0, 4.0);
    let pin = gen::cylinder_mesh(6.0, 6.0, 20.0, 24).translated(Vec3::new(0.0, 0.0, 12.0));
    let before = beside(&plate, &pin, Vec3::ZERO);
    let found = reassemble(&before, &plan());
    assert_eq!(found.parts.len(), 2);
    assert_eq!(found.groups, vec![vec![0, 1]]);
    let apart = reassemble(&before, &Reassemble { group_touching: false, ..plan() });
    assert_eq!(apart.groups, vec![vec![0], vec![1]]);
}

#[test]
fn a_plate_with_a_hole_is_not_a_box() {
    // Every corner of a drilled plate lies on its bounding box, so a corners-only check loses the hole.
    let plate = gen::box_mesh(40.0, 40.0, 6.0);
    let drill = gen::cylinder_mesh(12.0, 12.0, 20.0, 24);
    let before = crate::csg_bsp::subtract(&plate, &drill);
    let found = reassemble(&before, &plan());
    assert_eq!(shapes(&found), vec![Shape::Mesh], "a drilled plate came back as a solid");
}

#[test]
fn a_body_nothing_fits_keeps_its_triangles() {
    let before = gen::torus_mesh(30.0, 8.0, 360.0, 16);
    let found = reassemble(&before, &plan());
    assert_eq!(shapes(&found), vec![Shape::Mesh]);
    assert_eq!(found.parts[0].mesh.triangle_count(), before.weld().triangle_count());
    assert!(found.is_nothing(), "a mesh that came back a mesh is not worth doing");
}

#[test]
fn recognition_can_be_turned_off() {
    let before = gen::box_mesh(10.0, 10.0, 10.0);
    let found = reassemble(&before, &Reassemble { recognise: false, ..plan() });
    assert_eq!(shapes(&found), vec![Shape::Mesh]);
}

#[test]
fn the_cap_keeps_the_biggest_bodies_and_holds_the_rest() {
    let mut before = gen::box_mesh(40.0, 40.0, 40.0);
    for i in 0..6 {
        before.append(&gen::box_mesh(2.0, 2.0, 2.0).translated(Vec3::new(100.0 + 10.0 * f64::from(i), 0.0, 0.0)));
    }
    let found = reassemble(&before, &Reassemble { max_objects: 3, ..plan() });
    assert_eq!(found.parts.len(), 3, "the cap was not held to");
    assert_eq!(found.rest_bodies, 4);
    assert!(found.rest.is_some());
    // The largest body gets the node, which is why bodies are ordered before the cap.
    match found.parts[0].shape {
        Shape::Box { width, .. } => assert!((width - 40.0).abs() < 1e-6),
        other => panic!("the biggest body came back as {other:?}"),
    }
    assert_eq!(found.objects(), 4);
}

#[test]
fn a_loose_tolerance_does_not_make_a_cylinder_of_a_box() {
    let before = gen::box_mesh(20.0, 20.0, 20.0);
    let found = reassemble(&before, &Reassemble { tolerance: 3.0, ..plan() });
    assert!(matches!(found.parts[0].shape, Shape::Box { .. }), "{:?}", found.parts[0].shape);
}

#[test]
fn a_rotation_that_comes_back_is_the_rotation_that_went_in() {
    // The written angles must read back as the same rotation the mesh transform performs.
    for turn in [Vec3::ZERO, Vec3::new(90.0, 0.0, 0.0), Vec3::new(17.0, -43.0, 88.0), Vec3::new(0.0, 90.0, 0.0)] {
        let frame = frame::Frame {
            x: Vec3::new(1.0, 0.0, 0.0).rotate_xyz_deg(turn),
            y: Vec3::new(0.0, 1.0, 0.0).rotate_xyz_deg(turn),
            z: Vec3::new(0.0, 0.0, 1.0).rotate_xyz_deg(turn),
        };
        let read = frame.rotation_deg();
        for probe in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)] {
            let there = probe.rotate_xyz_deg(turn);
            let back = probe.rotate_xyz_deg(read);
            assert!((there - back).length() < 1e-9, "{turn:?} read back as {read:?}");
        }
    }
}

#[test]
fn a_turned_cylinder_comes_back_turned() {
    let turn = Vec3::new(-25.0, 40.0, 15.0);
    let at = Vec3::new(3.0, 60.0, -12.0);
    let before = gen::cylinder_mesh(9.0, 9.0, 30.0, 24).transformed(at, turn);
    let found = reassemble(&before, &plan());
    match found.parts[0].shape {
        Shape::Cylinder { diameter, height, segments } => {
            assert_eq!(segments, 24);
            assert!((diameter - 9.0).abs() < 1e-6 && (height - 30.0).abs() < 1e-6, "{diameter} by {height}");
        }
        other => panic!("a turned cylinder came back as {other:?}"),
    }
    assert!((found.parts[0].centre - at).length() < 1e-6);
    assert!(worst(&before, &found) < 1e-6, "off by {}", worst(&before, &found));
}

#[test]
fn a_run_can_be_abandoned() {
    let before = gen::ellipsoid_mesh(20.0, 20.0, 20.0, 48);
    assert!(reassemble_until(&before, &plan(), &|| true).is_none());
}

#[test]
fn a_mesh_of_nothing_comes_back_as_nothing() {
    let found = reassemble(&Mesh::new(), &plan());
    assert!(found.parts.is_empty() && found.rest.is_none() && found.groups.is_empty());
    assert!(!found.is_nothing(), "an empty mesh has no one body to have come back unchanged");
}

#[test]
fn a_shape_is_found_however_nearly_it_fits_something_else() {
    // Regression: near-miss box fits of a slender cylinder once filled the list and the cylinder fit
    // was never measured, until fits were kept per shape.
    let before = gen::cylinder_mesh(6.0, 6.0, 20.0, 24);
    let found = reassemble(&before, &plan());
    assert_eq!(shapes(&found), vec![Shape::Cylinder { diameter: 6.0, height: 20.0, segments: 24 }]);
}

#[test]
fn an_assembly_of_a_plate_and_a_pin_comes_back_as_both() {
    let plate = gen::box_mesh(40.0, 40.0, 4.0);
    let pin = gen::cylinder_mesh(6.0, 6.0, 20.0, 24).translated(Vec3::new(0.0, 0.0, 12.0));
    let before = beside(&plate, &pin, Vec3::ZERO);
    let found = reassemble(&before, &plan());
    assert_eq!(
        shapes(&found),
        vec![
            Shape::Box { width: 40.0, depth: 40.0, height: 4.0 },
            Shape::Cylinder { diameter: 6.0, height: 20.0, segments: 24 },
        ]
    );
    assert!((found.parts[1].centre - Vec3::new(0.0, 0.0, 12.0)).length() < 1e-6);
    assert!(worst(&before, &found) < 1e-6);
}

#[test]
fn an_upright_shape_comes_back_unturned() {
    // Regression: taking whichever cap came first as Z left an upright pin turned 180 degrees about X.
    let before = gen::cylinder_mesh(6.0, 6.0, 20.0, 24).translated(Vec3::new(0.0, 0.0, 12.0));
    let found = reassemble(&before, &plan());
    assert_eq!(found.parts[0].rotation, Vec3::ZERO, "an upright cylinder came back turned");
    assert!((found.parts[0].centre - Vec3::new(0.0, 0.0, 12.0)).length() < 1e-9);
}

#[test]
fn a_plate_of_pins_comes_back_as_every_pin() {
    // One big body and many small ones, like a real imported assembly.
    let mut before = gen::box_mesh(200.0, 60.0, 6.0);
    for i in 0..20 {
        let at = Vec3::new(f64::from(i) * 9.0 - 85.0, 0.0, 13.0);
        before.append(&gen::cylinder_mesh(6.0, 6.0, 20.0, 24).translated(at));
    }
    let found = reassemble(&before, &plan());
    assert_eq!(found.parts.len(), 21);
    assert_eq!(found.recognised(), 21, "a pin went unrecognised");
    assert!(matches!(found.parts[0].shape, Shape::Box { .. }), "the plate is not first");
    assert_eq!(found.groups, vec![(0..21).collect::<Vec<usize>>()], "the pins stand on the plate, so it is one group");
    assert!(worst(&before, &found) < 1e-6);
}

#[test]
fn a_shape_survives_being_stored_as_f32() {
    // Project files store positions as `f32`. Regression: a cap's centre vertex read back slightly
    // off-axis was counted as a side, so a hexagonal prism became seven-sided and then nothing.
    let build = || {
        gen::regular_prism_mesh(6, 40.0, 50.0, false)
            .transformed(Vec3::new(0.0, 230.0, 40.0), Vec3::new(-62.0, 30.0, 0.0))
    };
    let mut stored = build();
    for p in stored.positions.iter_mut() {
        *p = Vec3::new(f64::from(p.x as f32), f64::from(p.y as f32), f64::from(p.z as f32));
    }
    let found = reassemble(&stored, &plan());
    match found.parts[0].shape {
        Shape::Prism { sides, diameter, height } => {
            assert_eq!(sides, 6, "the cap's own centre vertex was counted as a side");
            assert!((diameter - 40.0).abs() < 1e-3 && (height - 50.0).abs() < 1e-3, "{diameter} by {height}");
        }
        other => panic!("a stored hexagonal prism came back as {other:?}"),
    }
    assert!(worst(&build(), &found) < 1e-3);
}
