use crate::feature::{extrude, revolve, Extrude};
use simple3d_geom::push_pull::Outline;
use std::f64::consts::PI;

fn square(lo: [f64; 2], hi: [f64; 2]) -> Outline {
    Outline::new(vec![lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]], Vec::new()).unwrap()
}

fn closed_and_outward(mesh: &simple3d_geom::Mesh) -> f64 {
    let welded = mesh.weld();
    assert_eq!(welded.manifold_issue(), None, "the solid is not closed");
    let volume = mesh.signed_volume();
    assert!(volume > 0.0, "the solid is inside out: {volume}");
    volume
}

#[test]
fn an_extrusion_runs_to_one_side_both_sides_and_symmetrically() {
    let outline = [square([0.0, 0.0], [10.0, 10.0])];
    let one = extrude(&outline, Extrude { from: 0.0, to: 5.0, draft: 0.0 }).unwrap();
    assert!((closed_and_outward(&one) - 500.0).abs() < 1e-6);
    let both = extrude(&outline, Extrude { from: -3.0, to: 5.0, draft: 0.0 }).unwrap();
    assert!((closed_and_outward(&both) - 800.0).abs() < 1e-6);
    let bounds = both.bounds().unwrap();
    assert!((bounds.0.z + 3.0).abs() < 1e-9 && (bounds.1.z - 5.0).abs() < 1e-9);
    assert!(extrude(&outline, Extrude { from: 0.0, to: 0.0, draft: 0.0 }).is_err());
}

#[test]
fn a_draft_narrows_the_walls_away_from_the_sketch_both_ways_and_too_steep_a_one_is_refused() {
    let outline = [square([0.0, 0.0], [10.0, 10.0])];
    let drafted = extrude(&outline, Extrude { from: -5.0, to: 5.0, draft: 10.0 }).unwrap();
    let volume = closed_and_outward(&drafted);
    assert!(volume < 1000.0 && volume > 600.0, "volume {volume}");
    let (lo, hi) = drafted.bounds().unwrap();
    assert!((lo.x - 0.0).abs() < 1e-9 && (hi.x - 10.0).abs() < 1e-9, "the widest is not at the sketch plane");
    assert!(extrude(&outline, Extrude { from: 0.0, to: 50.0, draft: 30.0 }).is_err(), "the walls crossed");
    let flared = extrude(&outline, Extrude { from: 0.0, to: 5.0, draft: -10.0 }).unwrap();
    assert!(closed_and_outward(&flared) > 500.0);
}

#[test]
fn a_ring_extrudes_with_its_hole() {
    let outer = square([0.0, 0.0], [10.0, 10.0]);
    let hole = square([3.0, 3.0], [7.0, 7.0]);
    let ring = Outline::new(outer.outer, vec![hole.outer]).unwrap();
    let mesh = extrude(&[ring], Extrude { from: 0.0, to: 2.0, draft: 5.0 }).unwrap();
    let volume = closed_and_outward(&mesh);
    assert!(volume < 2.0 * 84.0 && volume > 100.0, "volume {volume}");
}

#[test]
fn a_rectangle_beside_its_axis_revolves_into_a_tube_and_part_way_into_a_closed_wedge() {
    // From 2 to 5 off the Y axis, 4 tall: a tube of pi (5^2 - 2^2) 4.
    let outline = [square([2.0, 0.0], [5.0, 4.0])];
    let axis = ([0.0, 0.0], [0.0, 1.0]);
    let tube = revolve(&outline, axis, 360.0, 256).unwrap();
    let expected = PI * 21.0 * 4.0;
    let volume = closed_and_outward(&tube);
    assert!((volume - expected).abs() / expected < 0.01, "volume {volume}, expected {expected}");
    let quarter = revolve(&outline, axis, 90.0, 256).unwrap();
    let volume = closed_and_outward(&quarter);
    assert!((volume - expected / 4.0).abs() / expected < 0.01, "volume {volume}");
    // On the other side of the axis it still comes out the right way round.
    let mirrored = [square([-5.0, 0.0], [-2.0, 4.0])];
    assert!(closed_and_outward(&revolve(&mirrored, axis, 120.0, 128).unwrap()) > 0.0);
}

#[test]
fn a_profile_across_its_axis_is_refused_and_one_touching_it_closes_up() {
    let axis = ([0.0, 0.0], [0.0, 1.0]);
    assert!(revolve(&[square([-1.0, 0.0], [3.0, 4.0])], axis, 360.0, 64).is_err());
    let solid = revolve(&[square([0.0, 0.0], [3.0, 4.0])], axis, 360.0, 128).unwrap();
    let volume = closed_and_outward(&solid);
    assert!((volume - PI * 9.0 * 4.0).abs() / volume < 0.01, "volume {volume}");
}
