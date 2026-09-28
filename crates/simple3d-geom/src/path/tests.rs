use super::*;

fn v(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3::new(x, y, z)
}

#[test]
fn a_point_along_an_open_path_is_measured_by_length_not_by_corner() {
    // A short leg and a long one: halfway is on the long leg, not at the corner.
    let path = Path { points: vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(10.0, 30.0, 0.0)], closed: false };
    assert_eq!(path.length(), 40.0);
    let (at, heading) = path.at(20.0).unwrap();
    assert!((at - v(10.0, 10.0, 0.0)).length() < 1e-9, "{at:?}");
    assert!((heading - v(0.0, 1.0, 0.0)).length() < 1e-9);
    // The start faces along the first segment, and past the end stays on the end.
    assert!((path.at(0.0).unwrap().1 - v(1.0, 0.0, 0.0)).length() < 1e-9);
    assert!((path.at(99.0).unwrap().0 - v(10.0, 30.0, 0.0)).length() < 1e-9);
}

#[test]
fn an_open_path_puts_objects_on_both_ends_and_a_closed_one_spaces_them_round() {
    let square = |closed| Path {
        points: vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(10.0, 10.0, 0.0), v(0.0, 10.0, 0.0)],
        closed,
    };
    assert_eq!(square(false).spread(4), vec![0.0, 10.0, 20.0, 30.0]);
    // Closed, the fourth side counts and the last object does not land back on the first.
    assert_eq!(square(true).length(), 40.0);
    assert_eq!(square(true).spread(4), vec![0.0, 10.0, 20.0, 30.0]);
    assert_eq!(square(true).spread(8)[1], 5.0);
    assert_eq!(square(false).spread(1), vec![0.0]);
}

#[test]
fn edges_picked_in_any_order_and_direction_chain_into_one_closed_loop() {
    let (a, b, c, d) = (v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(10.0, 10.0, 0.0), v(0.0, 10.0, 0.0));
    let path = Path::from_edges(&[(a, b), (c, d), (c, b), (a, d)]);
    assert!(path.closed, "a loop picked whole did not close: {path:?}");
    assert_eq!(path.points.len(), 4);
    assert_eq!(path.length(), 40.0);
}

#[test]
fn edges_that_do_not_touch_are_bridged_to_the_nearest_end() {
    let path = Path::from_edges(&[(v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0)), (v(30.0, 0.0, 0.0), v(15.0, 0.0, 0.0))]);
    assert!(!path.closed);
    // The second edge is turned to meet the first: 0 -> 10, bridge to 15, on to 30.
    assert_eq!(path.points, vec![v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0), v(15.0, 0.0, 0.0), v(30.0, 0.0, 0.0)]);
    // A duplicate or zero-length pick adds nothing.
    let again = Path::from_edges(&[(v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0)), (v(10.0, 0.0, 0.0), v(0.0, 0.0, 0.0))]);
    assert_eq!(again.points.len(), 2);
}

#[test]
fn a_round_rim_comes_back_whole_and_a_box_edge_stops_at_its_corners() {
    // A 24-sided rim with a vertical edge down from every corner, as a cylinder's features are.
    let rim: Vec<Vec3> = (0..24)
        .map(|i| {
            let angle = (i as f64 * 15.0).to_radians();
            v(10.0 * angle.cos(), 10.0 * angle.sin(), 5.0)
        })
        .collect();
    let mut edges: Vec<(Vec3, Vec3)> = (0..24).map(|i| (rim[i], rim[(i + 1) % 24])).collect();
    edges.extend(rim.iter().map(|&p| (p, v(p.x, p.y, 0.0))));
    let run = smooth_run(&edges, edges[5]);
    assert_eq!(run.len(), 24, "the rim did not come back whole");
    assert!(Path::from_edges(&run).closed);

    // A box's top edge meets two edges at right angles at each corner, so it is on its own.
    let (a, b, c) = (v(0.0, 0.0, 10.0), v(10.0, 0.0, 10.0), v(10.0, 10.0, 10.0));
    let square = [(a, b), (b, c), (b, v(10.0, 0.0, 0.0)), (a, v(0.0, 0.0, 0.0))];
    assert_eq!(smooth_run(&square, (a, b)), vec![(a, b)]);
}
