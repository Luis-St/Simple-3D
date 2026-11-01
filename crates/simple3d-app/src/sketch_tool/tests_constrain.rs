use super::constrain::Rule;
use super::*;
use crate::app::tests::headless_app;
use crate::app::App;
use crate::view::View;
use simple3d_sketch::{ConstraintKind, Shape};

fn sketching() -> (App, View) {
    let mut app = headless_app();
    app.selection.clear();
    app.toggle_sketch_tool();
    app.place_sketch(Xform::from_translation(Vec3::ZERO), None);
    let view = View::new(app.scene.camera, app.viewport_rect);
    (app, view)
}

/// A drawing click at sketch point `at` as the pointer makes it, inferring what it would.
fn press(tool: &mut SketchTool, view: &View, at: [f64; 2]) {
    let cursor = tool.screen(view, at).unwrap();
    let (click, inferred) = tool.drawing_click(view, cursor, None).unwrap();
    tool.click_inferred(click, inferred);
}

fn line(tool: &mut SketchTool, view: &View, from: [f64; 2], to: [f64; 2]) -> CurveId {
    tool.set_draw(Draw::Line);
    press(tool, view, from);
    press(tool, view, to);
    tool.end_gesture();
    tool.sketch.curves.iter().map(|c| c.id).max().unwrap()
}

fn has(tool: &SketchTool, holds: impl Fn(&ConstraintKind) -> bool) -> bool {
    tool.sketch.constraints.iter().any(|c| holds(&c.kind))
}

#[test]
fn a_constraint_tool_takes_its_picks_one_click_at_a_time_and_goes_on_when_they_are_enough() {
    let (mut app, view) = sketching();
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.free = true;
    let a = line(tool, &view, [0.0, 0.0], [10.0, 2.0]);
    let b = line(tool, &view, [0.0, 10.0], [10.0, 14.0]);
    tool.choose(Draw::Constrain(Rule::Parallel));
    assert_eq!(tool.draw, Draw::Constrain(Rule::Parallel));
    let on = |tool: &SketchTool, p: [f64; 2]| tool.screen(&view, p).unwrap();
    let at = on(tool, [5.0, 1.0]);
    super::constrain_pick::constrain_at(tool, &view, at, Rule::Parallel);
    assert_eq!(tool.selection, vec![Pick::Curve(a)], "the first line was not picked");
    assert!(tool.sketch.constraints.is_empty(), "one line was enough");
    let at = on(tool, [5.0, 12.0]);
    super::constrain_pick::constrain_at(tool, &view, at, Rule::Parallel);
    assert!(has(tool, |k| matches!(*k, ConstraintKind::Parallel { a: p, b: q } if (p, q) == (a, b))));
    assert!(tool.selection.is_empty(), "the picks stayed after the constraint went on");
    assert_eq!(tool.draw, Draw::Constrain(Rule::Parallel), "the tool did not stay for the next one");
}

#[test]
fn choosing_a_constraint_puts_it_on_a_selection_that_fits_and_takes_up_the_tool_otherwise() {
    let (mut app, view) = sketching();
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.free = true;
    let a = line(tool, &view, [0.0, 0.0], [10.0, 1.0]);
    tool.set_draw(Draw::Select);
    tool.selection = vec![Pick::Curve(a)];
    tool.choose(Draw::Constrain(Rule::Horizontal));
    assert!(has(tool, |k| matches!(k, ConstraintKind::Horizontal { .. })), "not put straight on");
    assert_eq!(tool.draw, Draw::Select, "putting it on changed the tool");
    let point = tool.sketch.points[0].id;
    tool.selection = vec![Pick::Curve(a), Pick::Point(point)];
    tool.choose(Draw::Constrain(Rule::Parallel));
    assert_eq!(tool.draw, Draw::Constrain(Rule::Parallel));
    assert!(tool.selection.is_empty(), "a selection that can never fit was kept");
}

#[test]
fn where_a_dimension_is_placed_decides_whether_it_measures_true_length_across_or_up() {
    let (mut app, view) = sketching();
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.free = true;
    let slanted = line(tool, &view, [0.0, 0.0], [10.0, 10.0]);
    tool.selection = vec![Pick::Curve(slanted)];
    let measure = tool.measure().unwrap();
    let kind = |tool: &SketchTool, at| tool.dimension_kind(measure, at).unwrap().0;
    assert!(
        matches!(kind(tool, [5.0, 20.0]), ConstraintKind::HorizontalDistance { value, .. } if (value - 10.0).abs() < 1e-4)
    );
    assert!(matches!(kind(tool, [20.0, 5.0]), ConstraintKind::VerticalDistance { .. }));
    assert!(
        matches!(kind(tool, [2.0, 8.0]), ConstraintKind::Distance { value, .. } if (value - 200f64.sqrt()).abs() < 1e-4)
    );
    let circle = tool.change(|s| s.add_circle([40.0, 0.0], 3.0));
    tool.selection = vec![Pick::Curve(circle)];
    let round = tool.measure().unwrap();
    assert!(
        matches!(tool.dimension_kind(round, [45.0, 5.0]).unwrap().0, ConstraintKind::Diameter { value, .. } if (value - 6.0).abs() < 1e-9)
    );
}

#[test]
fn a_dimension_the_others_already_decide_goes_on_as_a_reference_dimension() {
    let (mut app, view) = sketching();
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(Draw::Rectangle);
    press(tool, &view, [0.0, 0.0]);
    press(tool, &view, [40.0, 30.0]);
    tool.set_draw(Draw::Dimension);
    let dimension = |tool: &mut SketchTool, on: [f64; 2], to: [f64; 2]| {
        let on = tool.screen(&view, on).unwrap();
        let first = super::dimension::dimension_at(tool, &view, on).0;
        let to = tool.screen(&view, to).unwrap();
        let (said, placed) = super::dimension::dimension_at(tool, &view, to);
        placed.unwrap_or_else(|| panic!("not placed: {first:?} then {said:?} with {:?}", tool.selection))
    };
    let bottom = dimension(tool, [20.0, 0.0], [20.0, -8.0]);
    assert!(!tool.sketch.constraint(bottom).unwrap().driven);
    let top = dimension(tool, [20.0, 30.0], [20.0, 38.0]);
    assert!(tool.sketch.constraint(top).unwrap().driven, "the top's length repeats the bottom's");
    assert!(tool.analysis.redundant.is_empty(), "a reference dimension is marked as fighting");
    tool.sketch.set_value(bottom, 50.0).unwrap();
    assert!((tool.sketch.constraint(top).unwrap().kind.value().unwrap() - 50.0).abs() < 1e-6);
}

#[test]
fn a_line_drawn_nearly_level_is_snapped_and_held_level_unless_drawn_freely() {
    let (mut app, view) = sketching();
    let tool = app.sketch_tool.as_mut().unwrap();
    let level = line(tool, &view, [0.0, 0.0], [10.0, 0.3]);
    let Some(Shape::Line { b, .. }) = tool.shape(level).cloned() else { panic!() };
    assert!(tool.sketch.at(b)[1].abs() < 1e-9, "the end was not snapped level");
    assert!(has(tool, |k| matches!(*k, ConstraintKind::Horizontal { line } if line == level)));
    tool.free = true;
    let loose = line(tool, &view, [0.0, 20.0], [10.0, 20.3]);
    assert!(!has(tool, |k| k.curves().contains(&loose)), "Ctrl still inferred a constraint");
}

#[test]
fn a_line_ending_on_a_circle_stays_on_it_and_one_square_to_the_last_is_held_square() {
    let (mut app, view) = sketching();
    let tool = app.sketch_tool.as_mut().unwrap();
    let circle = tool.change(|s| s.add_circle([30.0, 0.0], 5.0));
    let onto = line(tool, &view, [0.0, 7.0], [30.0 + 5.0 * 0.6, 5.0 * 0.8]);
    assert!(has(tool, |k| matches!(*k, ConstraintKind::OnCurve { curve, .. } if curve == circle)));
    // A chain: the second line leaves the first nearly square to it.
    tool.set_draw(Draw::Line);
    press(tool, &view, [0.0, 40.0]);
    press(tool, &view, [10.0, 45.0]);
    press(tool, &view, [10.0 - 4.0 + 0.15, 45.0 + 8.0]);
    tool.end_gesture();
    assert!(has(tool, |k| matches!(k, ConstraintKind::Perpendicular { .. })), "no perpendicular inferred");
    assert!(tool.sketch.curve(onto).is_some());
}
