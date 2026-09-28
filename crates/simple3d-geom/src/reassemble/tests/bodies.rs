//! How a mesh is divided into bodies and parts, and how a run is capped, turned off or abandoned.

use super::*;

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
