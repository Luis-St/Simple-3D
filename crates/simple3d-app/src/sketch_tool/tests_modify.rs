use super::*;
use crate::app::tests::headless_app;
use crate::app::App;
use simple3d_sketch::{ConstraintKind, Shape};

fn sketching() -> App {
    let mut app = headless_app();
    app.selection.clear();
    app.toggle_sketch_tool();
    app.place_sketch(Xform::from_translation(Vec3::ZERO), None);
    app
}

fn draw<'a>(app: &'a mut App, draw: Draw, clicks: &[[f64; 2]]) -> &'a mut SketchTool {
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(draw);
    for &at in clicks {
        let point = tool.sketch.points.iter().find(|p| (p.at[0] - at[0]).hypot(p.at[1] - at[1]) < 1e-9).map(|p| p.id);
        tool.click(Click { at, point });
    }
    tool.end_gesture();
    tool
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6
}

fn line_ends(tool: &SketchTool, id: CurveId) -> ([f64; 2], [f64; 2]) {
    let Shape::Line { a, b } = tool.sketch.curve(id).unwrap().shape else { panic!("not a line") };
    (tool.sketch.at(a), tool.sketch.at(b))
}

/// Run `edit` on the sketch and solve, as the tools' clicks do.
fn edit<T>(tool: &mut SketchTool, edit: impl FnOnce(&mut Sketch) -> T) -> T {
    let out = edit(&mut tool.sketch);
    assert!(tool.sketch.solve(), "the change does not solve");
    tool.refresh();
    out
}

#[test]
fn extend_runs_a_line_on_to_the_line_ahead_of_it() {
    let mut app = sketching();
    draw(&mut app, Draw::Line, &[[0.0, 0.0], [5.0, 0.0]]);
    let tool = draw(&mut app, Draw::Line, &[[10.0, -5.0], [10.0, 5.0]]);
    let short = tool.sketch.curves[0].id;
    edit(tool, |s| super::modify_ends::extend(s, short, [4.0, 0.0])).unwrap();
    assert!(near(line_ends(tool, short).1, [10.0, 0.0]), "ends at {:?}", line_ends(tool, short));
    assert!(tool.sketch.constraints.iter().any(|c| matches!(c.kind, ConstraintKind::OnCurve { .. })));
    let lone = draw(&mut app, Draw::Line, &[[0.0, 20.0], [5.0, 30.0]]).sketch.curves.last().unwrap().id;
    let tool = app.sketch_tool.as_mut().unwrap();
    assert!(super::modify_ends::extend(&mut tool.sketch, lone, [5.0, 30.0]).is_err(), "nothing lies ahead");
}

#[test]
fn split_cuts_a_line_in_two_that_stay_in_line() {
    let mut app = sketching();
    let tool = draw(&mut app, Draw::Line, &[[0.0, 0.0], [10.0, 0.0]]);
    let line = tool.sketch.curves[0].id;
    edit(tool, |s| super::modify_ends::split(s, line, [4.0, 0.3], None)).unwrap();
    assert_eq!(tool.sketch.curves.len(), 2);
    assert!(near(line_ends(tool, line).1, [4.0, 0.0]));
    assert!(tool.analysis.redundant.is_empty());
    assert!(super::modify_ends::split(&mut tool.sketch, line, [0.0, 0.0], None).is_err(), "not at an end");
}

#[test]
fn split_cuts_an_arc_into_two_on_one_circle() {
    let mut app = sketching();
    let tool = draw(&mut app, Draw::Arc, &[[0.0, 0.0], [10.0, 0.0], [-10.0, 0.0]]);
    let arc = tool.sketch.curves[0].id;
    edit(tool, |s| super::modify_ends::split(s, arc, [0.0, 12.0], None)).unwrap();
    assert_eq!(tool.sketch.curves.len(), 2);
    assert!(tool.sketch.points.iter().any(|p| near(p.at, [0.0, 10.0])), "cut where the click meets the circle");
    assert!(tool.analysis.redundant.is_empty());
}

#[test]
fn a_chamfer_cuts_a_rectangle_corner_off_straight() {
    let mut app = sketching();
    let tool = draw(&mut app, Draw::Rectangle, &[[0.0, 0.0], [20.0, 10.0]]);
    let corner = tool.sketch.points.iter().find(|p| near(p.at, [0.0, 0.0])).unwrap().id;
    let cut = edit(tool, |s| super::modify_ends::chamfer(s, corner, 3.0)).unwrap();
    let (a, b) = line_ends(tool, cut);
    assert!((near(a, [3.0, 0.0]) && near(b, [0.0, 3.0])) || (near(b, [3.0, 0.0]) && near(a, [0.0, 3.0])));
    assert_eq!(simple3d_sketch::regions::find(&tool.sketch, 64).list.len(), 1);
}

#[test]
fn an_offset_rectangle_is_the_same_rectangle_bigger_all_round() {
    let mut app = sketching();
    let tool = draw(&mut app, Draw::Rectangle, &[[0.0, 0.0], [20.0, 10.0]]);
    let bottom = tool.sketch.curves[0].id;
    let added = edit(tool, |s| super::modify_offset::offset(s, bottom, [10.0, -1.0], 2.0)).unwrap();
    assert_eq!(added, 4);
    for corner in [[-2.0, -2.0], [22.0, -2.0], [22.0, 12.0], [-2.0, 12.0]] {
        assert!(tool.sketch.points.iter().any(|p| near(p.at, corner)), "no corner at {corner:?}");
    }
    assert_eq!(simple3d_sketch::regions::find(&tool.sketch, 64).list.len(), 2, "a frame round a rectangle");
    assert!(tool.analysis.redundant.is_empty());
}

#[test]
fn an_offset_open_chain_of_a_line_and_an_arc_follows_both() {
    let mut app = sketching();
    draw(&mut app, Draw::Line, &[[0.0, 0.0], [10.0, 0.0]]);
    let tool = draw(&mut app, Draw::TangentArc, &[[10.0, 0.0], [10.0, 10.0]]);
    let line = tool.sketch.curves[0].id;
    let added = edit(tool, |s| super::modify_offset::offset(s, line, [5.0, 1.0], 1.0)).unwrap();
    assert_eq!(added, 2);
    // Inside the turn: the arc's copy is a radius of 4 about the same centre.
    assert!(tool.sketch.points.iter().any(|p| near(p.at, [10.0, 1.0])), "the joint did not follow");
    assert!(tool.sketch.points.iter().any(|p| near(p.at, [10.0, 9.0])));
    assert!(tool.analysis.redundant.is_empty());
}

#[test]
fn mirror_copies_the_selection_across_a_line_and_holds_it_there() {
    let mut app = sketching();
    draw(&mut app, Draw::Line, &[[0.0, -10.0], [0.0, 10.0]]);
    let tool = draw(&mut app, Draw::Line, &[[2.0, 0.0], [4.0, 3.0]]);
    let (axis, piece) = (tool.sketch.curves[0].id, tool.sketch.curves[1].id);
    edit(tool, |s| super::modify_copy::mirror(s, &[piece, axis], axis)).unwrap();
    let copy = tool.sketch.curves.last().unwrap().id;
    let (a, b) = line_ends(tool, copy);
    assert!(near(a, [-2.0, 0.0]) && near(b, [-4.0, 3.0]), "mirrored to {a:?} {b:?}");
    let held = tool.sketch.constraints.iter().filter(|c| matches!(c.kind, ConstraintKind::Symmetric { .. })).count();
    assert_eq!(held, 2);
    assert!(tool.sketch.drag(&[(tool.sketch.points[2].id, [3.0, 1.0])]));
    // Wherever the drag leaves the axis, the image is the dragged end mirrored across it.
    let ((p, q), end, image) = (line_ends(tool, axis), line_ends(tool, piece).0, line_ends(tool, copy).0);
    let d = [(q[0] - p[0]) / (q[0] - p[0]).hypot(q[1] - p[1]), (q[1] - p[1]) / (q[0] - p[0]).hypot(q[1] - p[1])];
    let along = (end[0] - p[0]) * d[0] + (end[1] - p[1]) * d[1];
    let foot = [p[0] + d[0] * along, p[1] + d[1] * along];
    assert!(near(image, [2.0 * foot[0] - end[0], 2.0 * foot[1] - end[1]]), "the image did not follow");
}

#[test]
fn patterns_copy_the_selection_with_its_own_constraints() {
    let mut app = sketching();
    let tool = draw(&mut app, Draw::Rectangle, &[[0.0, 0.0], [4.0, 2.0]]);
    let curves: Vec<CurveId> = tool.sketch.curves.iter().map(|c| c.id).collect();
    tool.settings.pattern_count = 3;
    let settings = std::mem::take(&mut tool.settings);
    edit(tool, |s| super::modify_copy::linear_pattern(s, &curves, [0.0, 0.0], [10.0, 0.0], &settings));
    assert_eq!(tool.sketch.curves.len(), 12);
    assert!(tool.sketch.points.iter().any(|p| near(p.at, [24.0, 2.0])));
    let horizontal =
        tool.sketch.constraints.iter().filter(|c| matches!(c.kind, ConstraintKind::Horizontal { .. })).count();
    assert_eq!(horizontal, 6, "the copies are rectangles too");
    let mut app = sketching();
    let tool = draw(&mut app, Draw::Circle, &[[10.0, 0.0], [11.0, 0.0]]);
    let circle = tool.sketch.curves[0].id;
    let mut settings = super::settings::Settings::default();
    settings.pattern_count = 4;
    edit(tool, |s| super::modify_copy::circular_pattern(s, &[circle], [0.0, 0.0], &settings));
    for centre in [[0.0, 10.0], [-10.0, 0.0], [0.0, -10.0]] {
        assert!(tool.sketch.points.iter().any(|p| near(p.at, centre)), "no copy at {centre:?}");
    }
}

#[test]
fn move_rotate_and_scale_place_the_selection_by_their_clicks() {
    let mut app = sketching();
    let tool = draw(&mut app, Draw::Line, &[[0.0, 0.0], [10.0, 0.0]]);
    let line = tool.sketch.curves[0].id;
    let points: Vec<PointId> = tool.sketch.points.iter().map(|p| p.id).collect();
    let curves = [line];
    let picked = (curves.as_slice(), points.as_slice());
    edit(tool, |s| super::modify_copy::transform(s, Draw::Move, picked, &[[0.0, 0.0], [1.0, 2.0]]));
    assert!(near(line_ends(tool, line).0, [1.0, 2.0]));
    edit(tool, |s| super::modify_copy::transform(s, Draw::Rotate, picked, &[[1.0, 2.0], [2.0, 2.0], [1.0, 3.0]]));
    assert!(near(line_ends(tool, line).1, [1.0, 12.0]), "turned a quarter: {:?}", line_ends(tool, line));
    edit(tool, |s| super::modify_copy::transform(s, Draw::Scale, picked, &[[1.0, 2.0], [1.0, 3.0], [1.0, 4.0]]));
    assert!(near(line_ends(tool, line).1, [1.0, 22.0]), "doubled: {:?}", line_ends(tool, line));
}

#[test]
fn an_offset_rounded_rectangle_stays_smooth_without_repeating_constraints() {
    let mut app = sketching();
    let tool = draw(&mut app, Draw::RoundedRectangle, &[[0.0, 0.0], [20.0, 10.0]]);
    let bottom = tool.sketch.curves[0].id;
    let added = edit(tool, |s| super::modify_offset::offset(s, bottom, [10.0, -1.0], 2.0)).unwrap();
    assert_eq!(added, 8);
    assert!(tool.analysis.redundant.is_empty(), "the copies' constraints repeat each other");
    assert!(tool.sketch.points.iter().any(|p| near(p.at, [10.0, -2.0]) || near(p.at, [2.0, -2.0])));
    assert_eq!(simple3d_sketch::regions::find(&tool.sketch, 64).list.len(), 2);
}
