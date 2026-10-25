use super::rectangle;
use crate::regions::find;
use crate::{Shape, Sketch};

fn near(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() < tolerance
}

#[test]
fn a_closed_rectangle_is_one_region_and_an_open_one_none() {
    let mut sketch = Sketch::default();
    let (_, lines) = rectangle(&mut sketch, [0.0, 0.0], [10.0, 5.0]);
    let regions = find(&sketch, 64);
    assert_eq!(regions.list.len(), 1);
    assert!(near(regions.list[0].area, 50.0, 1e-9));
    sketch.remove_curve(lines[2]);
    assert!(find(&sketch, 64).list.is_empty(), "an open outline enclosed something");
}

#[test]
fn a_circle_inside_a_rectangle_makes_a_ring_and_a_disc_that_merge_back_into_one() {
    let mut sketch = Sketch::default();
    rectangle(&mut sketch, [0.0, 0.0], [20.0, 20.0]);
    sketch.add_circle([10.0, 10.0], 4.0);
    let regions = find(&sketch, 128);
    assert_eq!(regions.list.len(), 2);
    let ring = regions.at([1.0, 1.0]).unwrap();
    let disc = regions.at([10.0, 10.0]).unwrap();
    assert_ne!(ring, disc);
    assert_eq!(regions.list[ring].holes.len(), 1);
    let disc_area = regions.list[disc].area;
    assert!(near(disc_area, std::f64::consts::PI * 16.0, 0.1), "disc area {disc_area}");
    assert!(near(regions.list[ring].area + disc_area, 400.0, 1e-6));
    let ring_only = regions.outlines(&[ring]);
    assert_eq!(ring_only.len(), 1);
    assert_eq!(ring_only[0].holes.len(), 1, "the ring lost its hole");
    let both = regions.outlines(&[ring, disc]);
    assert_eq!(both.len(), 1);
    assert!(both[0].holes.is_empty(), "the picked disc did not fill the hole");
    assert!(near(both[0].area(), 400.0, 1e-6));
}

#[test]
fn a_line_across_a_circle_splits_it_and_a_line_ending_on_it_closes_a_region() {
    let mut sketch = Sketch::default();
    let circle = sketch.add_circle([0.0, 0.0], 5.0);
    sketch.add_line([-8.0, 1.0], [8.0, 1.0]);
    assert_eq!(find(&sketch, 64).list.len(), 2);

    let mut sketch = Sketch::default();
    sketch.add_circle([0.0, 0.0], 5.0);
    // A chord from one point on the circle to another, ends exactly on the curve.
    let s = 0.5_f64.sqrt() * 5.0;
    sketch.add_line([-s, s], [s, s]);
    let regions = find(&sketch, 64);
    assert_eq!(regions.list.len(), 2, "the chord did not close a cap");
    let _ = circle;
}

#[test]
fn a_pick_is_found_again_after_the_sketch_is_edited() {
    let mut sketch = Sketch::default();
    let (corners, _) = rectangle(&mut sketch, [0.0, 0.0], [20.0, 20.0]);
    sketch.add_circle([10.0, 10.0], 4.0);
    let regions = find(&sketch, 64);
    let ring = regions.at([1.0, 1.0]).unwrap();
    let pick = regions.pick(ring, [1.0, 1.0]);
    sketch.point_mut(corners[2]).unwrap().at = [40.0, 30.0];
    let moved = find(&sketch, 64);
    let again = moved.resolve(&pick).expect("the ring was lost");
    assert_eq!(moved.list[again].holes.len(), 1, "the pick moved to the disc");
}

#[test]
fn a_closed_spline_and_a_control_point_spline_closed_by_a_line_enclose_regions() {
    let mut sketch = Sketch::default();
    let points = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]].map(|at| sketch.add_point(at));
    sketch.add_curve(Shape::Spline { points: points.to_vec(), closed: true }, false);
    let regions = find(&sketch, 64);
    assert_eq!(regions.list.len(), 1);
    assert!(regions.list[0].area > 80.0, "the spline does not run round its points");

    let mut sketch = Sketch::default();
    let points = [[0.0, 0.0], [5.0, 10.0], [10.0, 0.0]].map(|at| sketch.add_point(at));
    sketch.add_curve(Shape::Bspline { points: points.to_vec(), closed: false }, false);
    sketch.add_curve(Shape::Line { a: points[2], b: points[0] }, false);
    let regions = find(&sketch, 64);
    assert_eq!(regions.list.len(), 1);
    let area = regions.list[0].area;
    assert!(area > 20.0 && area < 50.0, "a control-point spline should stay inside its points: {area}");
}

#[test]
fn construction_curves_bound_nothing() {
    let mut sketch = Sketch::default();
    let (_, lines) = rectangle(&mut sketch, [0.0, 0.0], [10.0, 5.0]);
    sketch.curve_mut(lines[1]).unwrap().construction = true;
    assert!(find(&sketch, 64).list.is_empty());
}
