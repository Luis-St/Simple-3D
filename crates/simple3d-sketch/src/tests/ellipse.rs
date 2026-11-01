use crate::{ConstraintKind, Ellipse, Shape, Sketch};

#[test]
fn a_point_held_on_an_ellipse_is_pulled_onto_it() {
    let mut sketch = Sketch::default();
    let (center, major) = (sketch.add_point([0.0, 0.0]), sketch.add_point([10.0, 0.0]));
    let ellipse = sketch.add_curve(Shape::Ellipse { center, major, minor: 4.0 }, false);
    let point = sketch.add_point([3.0, 7.0]);
    sketch.constrain(ConstraintKind::OnCurve { point, curve: ellipse }).expect("a point can be held on an ellipse");
    let Shape::Ellipse { minor, .. } = sketch.curve(ellipse).unwrap().shape else { panic!() };
    let e = Ellipse::new(sketch.at(center), sketch.at(major), minor);
    assert!(e.residual(sketch.at(point)).abs() < 1e-7, "the point is off the ellipse");
}

#[test]
fn an_elliptical_arc_keeps_its_ends_on_the_ellipse_when_its_axis_is_dragged() {
    let mut sketch = Sketch::default();
    let (center, major) = (sketch.add_point([0.0, 0.0]), sketch.add_point([10.0, 0.0]));
    let e = Ellipse::new([0.0, 0.0], [10.0, 0.0], 5.0);
    let (start, end) = (sketch.add_point(e.at(0.3)), sketch.add_point(e.at(2.0)));
    let arc = sketch.add_curve(Shape::EllipseArc { center, major, minor: 5.0, start, end }, false);
    assert!(sketch.drag(&[(major, [12.0, 3.0])]));
    let Shape::EllipseArc { minor, .. } = sketch.curve(arc).unwrap().shape else { panic!() };
    let e = Ellipse::new(sketch.at(center), sketch.at(major), minor);
    for p in [start, end] {
        assert!(e.residual(sketch.at(p)).abs() < 1e-7, "an end came off the ellipse");
    }
    // Free: the centre, the major point, the width and where each end sits along it.
    assert_eq!(sketch.analyse().dof, 7);
}

#[test]
fn a_closed_ellipse_bounds_a_region_and_an_arc_closed_by_a_line_does_too() {
    let mut sketch = Sketch::default();
    let (center, major) = (sketch.add_point([0.0, 0.0]), sketch.add_point([10.0, 0.0]));
    sketch.add_curve(Shape::Ellipse { center, major, minor: 4.0 }, false);
    let regions = crate::regions::find(&sketch, 64);
    assert_eq!(regions.list.len(), 1);
    let mut sketch = Sketch::default();
    let (center, major) = (sketch.add_point([0.0, 0.0]), sketch.add_point([10.0, 0.0]));
    let (start, end) = (sketch.add_point([10.0, 0.0]), sketch.add_point([-10.0, 0.0]));
    sketch.add_curve(Shape::EllipseArc { center, major, minor: 4.0, start, end }, false);
    sketch.add_curve(Shape::Line { a: end, b: start }, false);
    assert_eq!(crate::regions::find(&sketch, 64).list.len(), 1, "the half ellipse and its chord close a region");
}
