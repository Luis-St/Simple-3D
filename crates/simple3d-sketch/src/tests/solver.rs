use super::rectangle;
use crate::solver::Refusal;
use crate::{distance, ConstraintKind, Shape, Sketch};

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn a_rectangle_is_squared_up_and_sized_by_its_constraints() {
    let mut sketch = Sketch::default();
    let (c, l) = rectangle(&mut sketch, [0.0, 0.0], [10.0, 5.0]);
    sketch.point_mut(c[2]).unwrap().at = [11.0, 6.0];
    for kind in [
        ConstraintKind::Fix { point: c[0], at: [0.0, 0.0] },
        ConstraintKind::Horizontal { line: l[0] },
        ConstraintKind::Horizontal { line: l[2] },
        ConstraintKind::Vertical { line: l[1] },
        ConstraintKind::Vertical { line: l[3] },
        ConstraintKind::Distance { a: c[0], b: c[1], value: 20.0 },
        ConstraintKind::Distance { a: c[1], b: c[2], value: 8.0 },
    ] {
        sketch.constrain(kind).expect("each constraint should be taken");
    }
    let top = sketch.at(c[2]);
    assert!(near(top[0], 20.0) && near(top[1], 8.0), "the far corner is at {top:?}");
    let analysis = sketch.analyse();
    assert_eq!(analysis.dof, 0, "a fully dimensioned rectangle has nothing left to move");
    assert!(analysis.free_points.is_empty() && analysis.free_curves.is_empty());
}

#[test]
fn an_unconstrained_point_is_reported_free_and_a_held_one_is_not() {
    let mut sketch = Sketch::default();
    let line = sketch.add_line([0.0, 0.0], [10.0, 0.0]);
    let Shape::Line { a, b } = sketch.curve(line).unwrap().shape else { panic!() };
    sketch.constrain(ConstraintKind::Fix { point: a, at: [0.0, 0.0] }).unwrap();
    let analysis = sketch.analyse();
    assert_eq!(analysis.dof, 2);
    assert!(!analysis.free_points.contains(&a) && analysis.free_points.contains(&b));
    assert!(analysis.free_curves.contains(&line));
}

#[test]
fn a_constraint_that_only_repeats_others_is_refused_with_them_named() {
    let mut sketch = Sketch::default();
    let (c, l) = rectangle(&mut sketch, [0.0, 0.0], [10.0, 5.0]);
    let first = sketch.constrain(ConstraintKind::Horizontal { line: l[0] }).unwrap();
    let refused = sketch.constrain(ConstraintKind::VerticalDistance { a: c[0], b: c[1], value: 0.0 });
    assert_eq!(refused, Err(Refusal::Redundant(vec![first])));
    assert_eq!(sketch.constraints.len(), 1, "the refused constraint was kept");
}

#[test]
fn a_constraint_that_cannot_be_met_leaves_the_sketch_as_it_was() {
    let mut sketch = Sketch::default();
    let line = sketch.add_line([0.0, 0.0], [10.0, 0.0]);
    let Shape::Line { a, b } = sketch.curve(line).unwrap().shape else { panic!() };
    sketch.constrain(ConstraintKind::Fix { point: a, at: [0.0, 0.0] }).unwrap();
    sketch.constrain(ConstraintKind::Fix { point: b, at: [10.0, 0.0] }).unwrap();
    let before = sketch.clone();
    assert!(sketch.constrain(ConstraintKind::Distance { a, b, value: 3.0 }).is_err());
    assert_eq!(sketch, before);
}

#[test]
fn dragging_a_point_keeps_its_dimension_and_moves_it_along_the_free_direction() {
    let mut sketch = Sketch::default();
    let line = sketch.add_line([0.0, 0.0], [10.0, 0.0]);
    let Shape::Line { a, b } = sketch.curve(line).unwrap().shape else { panic!() };
    sketch.constrain(ConstraintKind::Fix { point: a, at: [0.0, 0.0] }).unwrap();
    sketch.constrain(ConstraintKind::Distance { a, b, value: 10.0 }).unwrap();
    assert!(sketch.drag(&[(b, [0.0, 20.0])]));
    let end = sketch.at(b);
    assert!(near(distance(end, [0.0, 0.0]), 10.0), "the length changed to {}", distance(end, [0.0, 0.0]));
    assert!(end[1] > 9.0, "the end did not swing toward the pointer: {end:?}");
}

#[test]
fn a_dimension_can_be_retyped() {
    let mut sketch = Sketch::default();
    let line = sketch.add_line([0.0, 0.0], [10.0, 0.0]);
    let Shape::Line { a, b } = sketch.curve(line).unwrap().shape else { panic!() };
    let id = sketch.constrain(ConstraintKind::Distance { a, b, value: 10.0 }).unwrap();
    sketch.set_value(id, 25.0).unwrap();
    assert!(near(distance(sketch.at(a), sketch.at(b)), 25.0));
}

#[test]
fn a_line_is_made_tangent_to_a_circle_and_an_arc_runs_on_from_a_line_smoothly() {
    let mut sketch = Sketch::default();
    let circle = sketch.add_circle([0.0, 5.0], 2.0);
    let line = sketch.add_line([-10.0, 0.0], [10.0, 0.5]);
    sketch.constrain(ConstraintKind::Tangent { a: line, b: circle, inside: false }).unwrap();
    let Shape::Circle { center, radius } = sketch.curve(circle).unwrap().shape else { panic!() };
    let Shape::Line { a, b } = sketch.curve(line).unwrap().shape else { panic!() };
    let (p, q, c) = (sketch.at(a), sketch.at(b), sketch.at(center));
    let gap = crate::cross(crate::sub(q, p), crate::sub(c, p)).abs() / distance(p, q);
    assert!(near(gap, radius), "the centre is {gap} from the line, the radius {radius}");

    let mut sketch = Sketch::default();
    let line = sketch.add_line([0.0, 0.0], [10.0, 0.0]);
    let Shape::Line { b, .. } = sketch.curve(line).unwrap().shape else { panic!() };
    let center = sketch.add_point([10.0, 4.0]);
    let end = sketch.add_point([14.0, 4.5]);
    sketch.point_mut(center).unwrap().at = [10.5, 4.0];
    let arc = sketch.add_curve(Shape::Arc { center, start: b, end }, false);
    sketch.solve();
    sketch.constrain(ConstraintKind::Tangent { a: line, b: arc, inside: false }).unwrap();
    let radius = crate::sub(sketch.at(b), sketch.at(center));
    let along = crate::sub(sketch.at(b), sketch.at(sketch.points[0].id));
    assert!(crate::dot(radius, along).abs() < 1e-6, "the arc does not leave the line's end along it");
}

#[test]
fn a_spline_is_tangent_where_it_ends_on_a_line() {
    let mut sketch = Sketch::default();
    let line = sketch.add_line([0.0, 0.0], [10.0, 0.0]);
    let Shape::Line { a, b } = sketch.curve(line).unwrap().shape else { panic!() };
    let p1 = sketch.add_point([12.0, 3.0]);
    let p2 = sketch.add_point([15.0, 6.0]);
    let spline = sketch.add_curve(Shape::Spline { points: vec![b, p1, p2], closed: false }, false);
    sketch.constrain(ConstraintKind::Tangent { a: line, b: spline, inside: false }).unwrap();
    let along = crate::sub(sketch.at(b), sketch.at(a));
    let leaving = crate::sub(sketch.at(p1), sketch.at(b));
    assert!(crate::cross(along, leaving).abs() < 1e-6, "the spline does not leave in the line's direction");
    let away = sketch.add_point([30.0, 30.0]);
    let refused = sketch.constrain(ConstraintKind::OnCurve { point: away, curve: spline });
    assert!(matches!(refused, Err(Refusal::Unsupported(_))));
}
