mod features;
mod regions;
mod solver;

use crate::{ConstraintKind, CurveId, PointId, Shape, Sketch};

/// A closed rectangle of four lines sharing their corners, its lines bottom, right, top, left.
pub(crate) fn rectangle(sketch: &mut Sketch, lo: [f64; 2], hi: [f64; 2]) -> ([PointId; 4], [CurveId; 4]) {
    let corners = [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]].map(|at| sketch.add_point(at));
    let lines = [0, 1, 2, 3].map(|i| sketch.add_curve(Shape::Line { a: corners[i], b: corners[(i + 1) % 4] }, false));
    (corners, lines)
}

#[test]
fn a_sketch_round_trips_through_its_stored_form() {
    let mut sketch = Sketch::default();
    let (corners, lines) = rectangle(&mut sketch, [0.0, 0.0], [10.0, 5.0]);
    sketch.add_circle([5.0, 2.5], 1.0);
    let p = [sketch.add_point([20.0, 0.0]), sketch.add_point([25.0, 5.0]), sketch.add_point([30.0, 0.0])];
    sketch.add_curve(Shape::Spline { points: p.to_vec(), closed: false }, true);
    sketch.push_constraint(ConstraintKind::Horizontal { line: lines[0] });
    sketch.push_constraint(ConstraintKind::Distance { a: corners[0], b: corners[1], value: 10.0 });
    sketch.axis = Some(lines[3]);
    let text = serde_json::to_string(&sketch).unwrap();
    let back: Sketch = serde_json::from_str(&text).unwrap();
    assert_eq!(back, sketch);
    assert!(back.is_consistent());
}

#[test]
fn removing_a_point_takes_its_curves_and_constraints_along() {
    let mut sketch = Sketch::default();
    let (corners, lines) = rectangle(&mut sketch, [0.0, 0.0], [10.0, 5.0]);
    sketch.push_constraint(ConstraintKind::Horizontal { line: lines[0] });
    sketch.push_constraint(ConstraintKind::Vertical { line: lines[2] });
    sketch.remove_point(corners[0]);
    assert_eq!(sketch.curves.len(), 2, "both lines on the corner should go");
    assert_eq!(sketch.constraints.len(), 1, "only the constraint on a removed line should go");
    assert!(sketch.is_consistent());
}

#[test]
fn merging_points_joins_curves_and_drops_what_collapses() {
    let mut sketch = Sketch::default();
    let line = sketch.add_line([0.0, 0.0], [10.0, 0.0]);
    let other = sketch.add_line([10.0, 0.1], [10.0, 10.0]);
    let Shape::Line { b, .. } = sketch.curve(line).unwrap().shape else { panic!() };
    let Shape::Line { a, .. } = sketch.curve(other).unwrap().shape else { panic!() };
    sketch.merge_points(b, a);
    assert_eq!(sketch.points.len(), 3);
    assert!(matches!(sketch.curve(other).unwrap().shape, Shape::Line { a, .. } if a == b));
    assert!(sketch.is_consistent());
}
