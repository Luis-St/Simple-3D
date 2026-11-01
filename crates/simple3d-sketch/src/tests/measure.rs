use super::rectangle;
use crate::solver::Refusal;
use crate::{ConstraintKind, Shape, Sketch};

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn a_reference_dimension_follows_the_sketch_and_holds_nothing() {
    let mut sketch = Sketch::default();
    let (c, _) = rectangle(&mut sketch, [0.0, 0.0], [10.0, 5.0]);
    let width = sketch.constrain(ConstraintKind::Distance { a: c[0], b: c[1], value: 10.0 }).unwrap();
    let dof = sketch.analyse().dof;
    // The same size again repeats the first, so it can only be kept as a reference.
    let again = ConstraintKind::Distance { a: c[1], b: c[0], value: 10.0 };
    assert!(matches!(sketch.clone().constrain(again.clone()), Err(Refusal::Redundant(_))));
    let reference = sketch.add_reference(again).unwrap();
    assert_eq!(sketch.analyse().dof, dof, "a reference dimension took a degree of freedom");
    assert!(sketch.analyse().redundant.is_empty(), "a reference dimension is marked as fighting");
    sketch.set_value(width, 14.0).unwrap();
    assert!(near(sketch.constraint(reference).unwrap().kind.value().unwrap(), 14.0), "it did not follow");
    assert!(sketch.set_value(reference, 3.0).is_err(), "a reference dimension drove the sketch");
    assert!(matches!(sketch.set_driven(reference, false), Err(Refusal::Redundant(_))));
    assert!(sketch.constraint(reference).unwrap().driven, "a refused switch was kept");
    sketch.remove_constraint(width);
    sketch.set_driven(reference, false).unwrap();
    sketch.set_value(reference, 6.0).unwrap();
    assert!(near(crate::distance(sketch.at(c[0]), sketch.at(c[1])), 6.0));
}

#[test]
fn a_diameter_sizes_a_circle_across_and_round_trips() {
    let mut sketch = Sketch::default();
    let circle = sketch.add_circle([0.0, 0.0], 3.0);
    let id = sketch.constrain(ConstraintKind::Diameter { curve: circle, value: 10.0 }).unwrap();
    let Shape::Circle { radius, .. } = sketch.curve(circle).unwrap().shape else { panic!() };
    assert!(near(radius, 5.0));
    assert!(near(sketch.constraint(id).unwrap().kind.measure(&sketch).unwrap(), 10.0));
    sketch.constraint_mut(id).unwrap().label = Some([4.0, 4.0]);
    let back: Sketch = serde_json::from_str(&serde_json::to_string(&sketch).unwrap()).unwrap();
    assert_eq!(back, sketch);
}
