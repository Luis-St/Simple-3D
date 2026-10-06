use super::*;
use crate::primitives::box_mesh;
use crate::vec3::Vec3;
use crate::{evaluate_boolean, BooleanOp};
use std::f64::consts::PI;

fn top_front(edges: &[FeatureEdge]) -> FeatureEdge {
    // The edge along X at y = -10, z = 10 of a 20 mm cube.
    *edges
        .iter()
        .find(|e| {
            let m = (e.a + e.b) / 2.0;
            (m.y + 10.0).abs() < 1e-9 && (m.z - 10.0).abs() < 1e-9 && m.x.abs() < 1e-9
        })
        .expect("no top front edge")
}

#[test]
fn a_cube_has_twelve_convex_edges_and_eight_corners() {
    let edges = feature_edges(&box_mesh(20.0, 20.0, 20.0));
    assert_eq!(edges.len(), 12);
    assert!(edges.iter().all(|e| e.convex && (e.length() - 20.0).abs() < 1e-9));
    assert!(edges.iter().all(|e| (e.opening() - PI / 2.0).abs() < 1e-9));
    let corners = Corner::find(&edges);
    assert_eq!(corners.len(), 8);
    assert!(corners.iter().all(|c| c.faces.len() == 3 && c.edges.len() == 3));
}

#[test]
fn a_rounded_and_a_chamfered_edge_take_away_what_they_should() {
    let cube = box_mesh(20.0, 20.0, 20.0);
    let edge = top_front(&feature_edges(&cube));
    let run = run_on(&cube, &edge, Treatment::Chamfer { distance: 2.0 });
    assert_eq!(run, [true, true], "a cube's edge ends in air at both ends");

    let chamfer = edge_solid(&edge, Treatment::Chamfer { distance: 2.0 }, run).unwrap();
    assert!(chamfer.manifold_issue().is_none());
    let cut = evaluate_boolean(BooleanOp::Difference, &[cube.clone(), chamfer]);
    assert!(cut.manifold_issue().is_none());
    assert!((cube.signed_volume() - cut.signed_volume() - 40.0).abs() < 1e-6, "{}", cut.signed_volume());

    let round = Treatment::Round { radius: 3.0, segments: 64 };
    let cutter = edge_solid(&edge, round, run).unwrap();
    let cut = evaluate_boolean(BooleanOp::Difference, &[cube.clone(), cutter]);
    assert!(cut.manifold_issue().is_none());
    let removed = cube.signed_volume() - cut.signed_volume();
    let expected = (9.0 - PI * 9.0 / 4.0) * 20.0;
    assert!((removed - expected).abs() < 0.05, "removed {removed}, a true arc removes {expected}");
}

#[test]
fn an_inside_edge_is_filled_and_does_not_run_on() {
    // An L: a floor and a wall, unioned.
    let floor = box_mesh(20.0, 20.0, 4.0).translated(Vec3::new(0.0, 0.0, 2.0));
    let wall = box_mesh(4.0, 20.0, 20.0).translated(Vec3::new(-8.0, 0.0, 10.0));
    let l = evaluate_boolean(BooleanOp::Union, &[floor, wall]);
    let edges = feature_edges(&l);
    let inside: Vec<&FeatureEdge> = edges.iter().filter(|e| !e.convex).collect();
    assert_eq!(inside.len(), 1, "{inside:?}");
    let edge = *inside[0];
    assert_eq!(run_on(&l, &edge, Treatment::Chamfer { distance: 2.0 }), [false, false]);
    let fill = edge_solid(&edge, Treatment::Chamfer { distance: 2.0 }, [false, false]).unwrap();
    let filled = evaluate_boolean(BooleanOp::Union, &[l.clone(), fill]);
    assert!(filled.manifold_issue().is_none());
    assert!((filled.signed_volume() - l.signed_volume() - 40.0).abs() < 1e-6, "{}", filled.signed_volume());
}

#[test]
fn a_cube_rounded_all_over_is_closed_and_near_its_true_volume() {
    let cube = box_mesh(20.0, 20.0, 20.0);
    let edges = feature_edges(&cube);
    let (r, k) = (3.0, 8);
    let round = Treatment::Round { radius: r, segments: k };
    let mut cutters: Vec<_> = edges.iter().map(|e| edge_solid(e, round, run_on(&cube, e, round)).unwrap()).collect();
    for corner in Corner::find(&edges) {
        cutters.push(corner_round(&corner, r, k).unwrap());
    }
    let cutter = evaluate_boolean(BooleanOp::Union, &cutters);
    let rounded = evaluate_boolean(BooleanOp::Difference, &[cube, cutter]);
    assert!(rounded.manifold_issue().is_none(), "{:?}", rounded.manifold_issue());
    // A cube of side 20 with every edge and corner rounded by r.
    let a = 20.0 - 2.0 * r;
    let truth = a * a * a + 6.0 * a * a * r + 3.0 * PI * r * r * a + 4.0 / 3.0 * PI * r * r * r;
    let v = rounded.signed_volume();
    assert!((v - truth).abs() / truth < 0.01, "volume {v}, a true rounding has {truth}");
    let (lo, hi) = rounded.bounds().unwrap();
    assert!((hi - lo - Vec3::new(20.0, 20.0, 20.0)).length() < 1e-6, "the faces moved");
}

#[test]
fn a_bevelled_corner_cuts_a_tetrahedron_off() {
    let cube = box_mesh(20.0, 20.0, 20.0);
    let corner = Corner::find(&feature_edges(&cube)).remove(0);
    let cutter = corner_chamfer(&corner, 3.0).unwrap();
    let cut = evaluate_boolean(BooleanOp::Difference, &[cube.clone(), cutter]);
    assert!(cut.manifold_issue().is_none());
    assert!((cube.signed_volume() - cut.signed_volume() - 4.5).abs() < 1e-6, "{}", cut.signed_volume());
}

#[test]
fn the_rim_of_a_cylinder_rounds_as_one_closed_surface() {
    let cylinder = crate::primitives::cylinder_mesh(20.0, 20.0, 10.0, 48);
    let edges = feature_edges(&cylinder);
    let rim: Vec<&FeatureEdge> = edges.iter().filter(|e| e.a.z > 4.9 && e.b.z > 4.9).collect();
    assert_eq!(rim.len(), 48);
    let round = Treatment::Round { radius: 2.0, segments: 6 };
    let cutters: Vec<_> = rim.iter().map(|e| edge_solid(e, round, run_on(&cylinder, e, round)).unwrap()).collect();
    let cutter = evaluate_boolean(BooleanOp::Union, &cutters);
    let rounded = evaluate_boolean(BooleanOp::Difference, &[cylinder.clone(), cutter]);
    assert!(rounded.manifold_issue().is_none(), "{:?}", rounded.manifold_issue());
    let removed = cylinder.signed_volume() - rounded.signed_volume();
    // A torus-like sliver: the corner area swept round the rim's circle, near enough.
    let expected = (4.0 - PI) * 2.0 * PI * (10.0 - 2.0 * (1.0 - 4.0 / (3.0 * (4.0 - PI))) * 1.0);
    assert!((removed - expected).abs() / expected < 0.15, "removed {removed}, expected about {expected}");
}

#[test]
fn the_sliver_of_an_edge_or_corner_is_exactly_what_its_cutter_takes() {
    let cube = box_mesh(20.0, 20.0, 20.0);
    let edges = feature_edges(&cube);
    let edge = top_front(&edges);
    let round = Treatment::Round { radius: 3.0, segments: 64 };
    // Running on into the air past both ends, the sliver still stops at the cube.
    let sliver = edge_sliver(&edge, round, [End::RunOn; 2]).unwrap();
    let (lo, hi) = sliver.bounds().unwrap();
    assert!(lo.x >= -10.0 - 1e-9 && hi.x <= 10.0 + 1e-9, "the sliver reaches past the cube: {lo:?} {hi:?}");
    assert!(sliver.manifold_issue().is_none());
    let expected = (9.0 - PI * 9.0 / 4.0) * 20.0;
    assert!((sliver.signed_volume() - expected).abs() < 0.05, "{}", sliver.signed_volume());

    let corner = &Corner::find(&edges)[0];
    let tip = corner_sliver(corner, Treatment::Chamfer { distance: 3.0 }).unwrap();
    assert!((tip.signed_volume() - 27.0 / 6.0).abs() < 1e-9, "{}", tip.signed_volume());
    let ball = corner_sliver(corner, round).unwrap();
    // The cube of the radius less an eighth of the ball.
    let expected = 27.0 - PI * 27.0 * 4.0 / 3.0 / 8.0;
    assert!((ball.signed_volume() - expected).abs() < 0.2, "{}", ball.signed_volume());
}

#[test]
fn an_edge_has_room_for_as_much_as_its_narrower_face() {
    // 20 wide, 4 high: the top's front edge has 4 of room down the front, 20 back across the top.
    let slab = box_mesh(20.0, 20.0, 4.0);
    let edges = feature_edges(&slab);
    let edge = *edges
        .iter()
        .find(|e| {
            let m = (e.a + e.b) / 2.0;
            (m.y + 10.0).abs() < 1e-9 && (m.z - 2.0).abs() < 1e-9
        })
        .unwrap();
    let room = edge_room(&slab, &edge);
    assert!((room - 4.0).abs() < 1e-9, "room {room}");
    assert!(edge_fits(&edge, Treatment::Round { radius: 4.0, segments: 8 }, room));
    assert!(!edge_fits(&edge, Treatment::Chamfer { distance: 4.5 }, room));
    // A half-round of the full height still cuts cleanly.
    let round = Treatment::Round { radius: 4.0, segments: 16 };
    let cut = evaluate_boolean(
        BooleanOp::Difference,
        &[slab.clone(), edge_solid(&edge, round, run_on(&slab, &edge, round)).unwrap()],
    );
    assert!(cut.manifold_issue().is_none());
    let corner = Corner::find(&edges).into_iter().next().unwrap();
    assert!(corner_fits(&corner, Treatment::Chamfer { distance: 3.9 }));
    assert!(!corner_fits(&corner, Treatment::Chamfer { distance: 4.0 }));
}

#[test]
fn two_outside_edges_meeting_at_an_inside_corner_are_mitred_into_one_seam() {
    // Three cubes of an L seen from above, the front right quarter missing: the two top edges along
    // the notch meet at the top of its inside corner, past which is material.
    let quarter = |x: f64, y: f64| box_mesh(20.0, 20.0, 20.0).translated(Vec3::new(x, y, 0.0));
    let l = evaluate_boolean(BooleanOp::Union, &[quarter(-10.0, 10.0), quarter(10.0, 10.0), quarter(-10.0, -10.0)]);
    let corner = Vec3::new(0.0, 0.0, 10.0);
    let edges: Vec<FeatureEdge> = feature_edges(&l)
        .into_iter()
        .filter(|e| e.convex && (e.a.z - 10.0).abs() < 1e-9 && (e.b.z - 10.0).abs() < 1e-9)
        .filter(|e| (e.a - corner).length() < 1e-9 || (e.b - corner).length() < 1e-9)
        .collect();
    assert_eq!(edges.len(), 2, "{edges:?}");
    let round = Treatment::Round { radius: 3.0, segments: 16 };
    // Unpicked, the corner leaves both edges flat; picked, it mitres them.
    let found = inside_corners(&feature_edges(&l));
    assert!(found.contains(&corner) && found.len() == 2, "the top and bottom of the notch: {found:?}");
    assert!((0..2).all(|i| !edge_ends(&l, &edges, i, round, &[]).iter().any(|e| matches!(e, End::Mitre(_)))));
    let ends: Vec<[End; 2]> = (0..2).map(|i| edge_ends(&l, &edges, i, round, &[corner])).collect();
    assert!(ends.iter().all(|e| e.iter().any(|end| matches!(end, End::Mitre(_)))), "{ends:?}");
    let mut operands = vec![l.clone()];
    operands.extend((0..2).map(|i| edge_solid_ends(&edges[i], round, ends[i]).unwrap()));
    let cut = evaluate_boolean(BooleanOp::Difference, &operands);
    assert!(cut.manifold_issue().is_none());
    // Just behind the corner, on the diagonal between the two rounds: flat-ended cutters left the
    // corner standing there as a spike.
    let behind = Vec3::new(-0.3, 0.3, 9.7);
    assert!(contains_point(&l, behind) && !contains_point(&cut, behind), "the corner's spike is still there");
    let mut flat = vec![l.clone()];
    flat.extend(edges.iter().map(|e| edge_solid(e, round, run_on(&l, e, round)).unwrap()));
    assert!(contains_point(&evaluate_boolean(BooleanOp::Difference, &flat), behind), "the spike was never there");
    // Further along the diagonal, past where the rounds meet, the top is untouched.
    assert!(contains_point(&cut, Vec3::new(-4.0, 4.0, 9.9)));
}
