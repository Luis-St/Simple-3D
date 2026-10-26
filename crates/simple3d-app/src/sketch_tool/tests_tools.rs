use super::*;
use crate::app::tests::{draw_one_frame, headless_app};
use crate::app::App;
use simple3d_sketch::{ConstraintKind, Shape};

fn click(tool: &mut SketchTool, at: [f64; 2]) {
    let point = tool.sketch.points.iter().find(|p| (p.at[0] - at[0]).hypot(p.at[1] - at[1]) < 1e-9).map(|p| p.id);
    tool.click(Click { at, point });
}

fn sketching() -> App {
    let mut app = headless_app();
    app.selection.clear();
    app.toggle_sketch_tool();
    app.place_sketch(Xform::from_translation(Vec3::ZERO), None);
    app
}

fn draw(app: &mut App, draw: Draw, clicks: &[[f64; 2]]) {
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(draw);
    for &at in clicks {
        click(tool, at);
    }
    tool.end_gesture();
}

fn lines(tool: &SketchTool) -> Vec<([f64; 2], [f64; 2])> {
    let mut out: Vec<([f64; 2], [f64; 2])> = tool
        .sketch
        .curves
        .iter()
        .filter(|c| !c.construction)
        .filter_map(|c| match c.shape {
            Shape::Line { a, b } => Some((tool.sketch.at(a), tool.sketch.at(b))),
            _ => None,
        })
        .collect();
    out.sort_by(|p, q| p.0[0].total_cmp(&q.0[0]).then(p.0[1].total_cmp(&q.0[1])));
    out
}

fn near(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6
}

fn length((a, b): ([f64; 2], [f64; 2])) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

#[test]
fn a_centre_rectangle_stays_centred_on_its_centre_point_when_a_corner_is_dragged() {
    let mut app = sketching();
    draw(&mut app, Draw::CenterRectangle, &[[0.0, 0.0], [10.0, 5.0]]);
    let tool = app.sketch_tool.as_mut().unwrap();
    assert_eq!(lines(tool).len(), 4);
    assert_eq!(simple3d_sketch::regions::find(&tool.sketch, 64).list.len(), 1, "the diagonals split it");
    let corner = tool.sketch.points.iter().find(|p| near(p.at, [10.0, 5.0])).unwrap().id;
    let centre = tool.sketch.points.iter().find(|p| near(p.at, [0.0, 0.0])).unwrap().id;
    assert!(tool.sketch.drag(&[(corner, [14.0, 8.0])]));
    // Wherever the drag leaves them, the opposite corner mirrors the dragged one through the centre.
    let (c, p) = (tool.sketch.at(centre), tool.sketch.at(corner));
    let opposite = [2.0 * c[0] - p[0], 2.0 * c[1] - p[1]];
    let at: Vec<_> = tool.sketch.points.iter().map(|p| p.at).collect();
    assert!((p[0] - 14.0).abs() < 0.01, "the corner did not move: {at:?}");
    assert!(tool.sketch.points.iter().any(|p| near(p.at, opposite)), "not centred: {at:?}");
}

#[test]
fn a_polygon_keeps_its_sides_equal_when_a_corner_is_dragged() {
    let mut app = sketching();
    app.sketch_tool.as_mut().unwrap().polygon_sides = 6;
    draw(&mut app, Draw::Polygon, &[[0.0, 0.0], [10.0, 0.0]]);
    let tool = app.sketch_tool.as_mut().unwrap();
    let sides = lines(tool);
    assert_eq!(sides.len(), 6);
    assert!(sides.iter().all(|&side| (length(side) - 10.0).abs() < 1e-6), "a regular hexagon's side is its radius");
    let corner = tool.sketch.points.iter().find(|p| near(p.at, [10.0, 0.0])).unwrap().id;
    assert!(tool.sketch.drag(&[(corner, [12.0, 1.0])]));
    let sides = lines(tool);
    let first = length(sides[0]);
    assert!(sides.iter().all(|&side| (length(side) - first).abs() < 1e-5), "the sides came apart: {sides:?}");
}

#[test]
fn three_points_make_a_circle_and_an_arc_through_them() {
    let mut app = sketching();
    draw(&mut app, Draw::ThreePointCircle, &[[0.0, 0.0], [10.0, 0.0], [5.0, 5.0]]);
    draw(&mut app, Draw::ThreePointArc, &[[20.0, 0.0], [30.0, 0.0], [25.0, -5.0]]);
    let tool = app.sketch_tool.as_ref().unwrap();
    let Shape::Circle { center, radius } = tool.sketch.curves[0].shape else { panic!("no circle") };
    assert!(near(tool.sketch.at(center), [5.0, 0.0]) && (radius - 5.0).abs() < 1e-9);
    let Shape::Arc { center, start, end } = tool.sketch.curves[1].shape else { panic!("no arc") };
    assert!(near(tool.sketch.at(center), [25.0, 0.0]));
    // Bent down through (25, -5), so counter-clockwise from the left end to the right.
    assert!(near(tool.sketch.at(start), [20.0, 0.0]) && near(tool.sketch.at(end), [30.0, 0.0]));
}

#[test]
fn trim_cuts_a_line_back_to_the_lines_crossing_it_or_away_altogether() {
    let mut app = sketching();
    draw(&mut app, Draw::Line, &[[-10.0, 0.0], [10.0, 0.0]]);
    draw(&mut app, Draw::Line, &[[-5.0, -5.0], [-5.0, 5.0]]);
    draw(&mut app, Draw::Line, &[[5.0, -5.0], [5.0, 5.0]]);
    let tool = app.sketch_tool.as_mut().unwrap();
    let across = tool.sketch.curves[0].id;
    tool.change(|sketch| super::modify::trim(sketch, across, [0.0, 0.0])).unwrap();
    let horizontal: Vec<_> = lines(tool).into_iter().filter(|(a, b)| a[1].abs() < 1e-6 && b[1].abs() < 1e-6).collect();
    assert_eq!(horizontal.len(), 2, "the middle was not cut out: {horizontal:?}");
    assert!(horizontal.iter().all(|&piece| (length(piece) - 5.0).abs() < 1e-6));
    assert!(tool.analysis.redundant.is_empty(), "the split left fighting constraints");
    // A line nothing crosses goes altogether.
    draw(&mut app, Draw::Line, &[[0.0, 20.0], [10.0, 20.0]]);
    let tool = app.sketch_tool.as_mut().unwrap();
    let lone = tool.sketch.curves.last().unwrap().id;
    tool.change(|sketch| super::modify::trim(sketch, lone, [5.0, 20.0])).unwrap();
    assert!(tool.sketch.curve(lone).is_none());
}

#[test]
fn trim_turns_a_circle_crossed_twice_into_the_arc_away_from_the_click() {
    let mut app = sketching();
    draw(&mut app, Draw::Circle, &[[0.0, 0.0], [10.0, 0.0]]);
    draw(&mut app, Draw::Line, &[[0.0, -20.0], [0.0, 20.0]]);
    let tool = app.sketch_tool.as_mut().unwrap();
    let circle = tool.sketch.curves[0].id;
    tool.change(|sketch| super::modify::trim(sketch, circle, [10.0, 0.0])).unwrap();
    let Shape::Arc { center, start, end } = tool.sketch.curve(circle).unwrap().shape else { panic!("not an arc") };
    let (c, s, e) = (tool.sketch.at(center), tool.sketch.at(start), tool.sketch.at(end));
    assert!(near(c, [0.0, 0.0]));
    // What is left is the left half: from the top round to the bottom.
    assert!(near(s, [0.0, 10.0]) && near(e, [0.0, -10.0]), "kept {s:?} to {e:?}");
}

#[test]
fn a_fillet_rounds_a_rectangle_corner_with_a_tangent_arc_of_its_radius() {
    let mut app = sketching();
    draw(&mut app, Draw::Rectangle, &[[0.0, 0.0], [20.0, 10.0]]);
    let tool = app.sketch_tool.as_mut().unwrap();
    let corner = tool.sketch.points.iter().find(|p| near(p.at, [0.0, 0.0])).unwrap().id;
    let before = tool.sketch.clone();
    let arc = super::modify::fillet(&mut tool.sketch, corner, 2.0).unwrap();
    assert!(tool.sketch.solve());
    let Shape::Arc { center, start, end } = tool.sketch.curve(arc).unwrap().shape else { panic!("no arc") };
    assert!(near(tool.sketch.at(center), [2.0, 2.0]));
    let ends = [tool.sketch.at(start), tool.sketch.at(end)];
    assert!(ends.iter().any(|&p| near(p, [2.0, 0.0])) && ends.iter().any(|&p| near(p, [0.0, 2.0])));
    assert!(tool.sketch.point(corner).is_none(), "the sharp corner stayed");
    assert!(tool.sketch.analyse().redundant.is_empty());
    assert_eq!(simple3d_sketch::regions::find(&tool.sketch, 64).list.len(), 1);
    tool.sketch = before;
    assert!(super::modify::fillet(&mut tool.sketch, corner, 50.0).is_err(), "a radius past the lines was taken");
}

#[test]
fn the_dimension_tool_dimensions_a_clicked_line_and_opens_its_popup() {
    let mut app = sketching();
    draw(&mut app, Draw::Line, &[[0.0, 0.0], [10.0, 0.0]]);
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(Draw::Dimension);
    let line = tool.sketch.curves[0].id;
    let at = tool.screen(&view, [5.0, 0.0]).unwrap();
    super::picks::dimension_at(tool, &view, at);
    assert!(tool.dimension_of(line).is_some(), "no length dimension");
    assert!(tool.inspect.is_some(), "the popup did not open");
    let dimension = tool.dimension_of(line).unwrap();
    assert!(tool.retype("value", dimension, 25.0).is_none());
    assert!((length(lines(tool)[0]) - 25.0).abs() < 1e-6);
}

#[test]
fn edits_from_one_popup_field_in_a_row_are_one_undo_step() {
    let mut app = sketching();
    draw(&mut app, Draw::Line, &[[0.0, 0.0], [10.0, 0.0]]);
    let tool = app.sketch_tool.as_mut().unwrap();
    let steps = tool.undo.len();
    let Shape::Line { b, .. } = tool.sketch.curves[0].shape else { unreachable!() };
    for x in [11.0, 12.0, 13.0] {
        assert!(tool.move_points("length-1", &[(b, [x, 0.0])]).is_none());
    }
    assert_eq!(tool.undo.len(), steps + 1);
    let existing = tool.holds(|k| matches!(k, ConstraintKind::Horizontal { .. }));
    tool.toggle(existing, ConstraintKind::Horizontal { line: tool.sketch.curves[0].id });
    assert_eq!(tool.undo.len(), steps + 2, "another kind of edit joined the field's step");
    tool.undo_step();
    tool.undo_step();
    assert!(near(tool.sketch.at(b), [10.0, 0.0]));
}

#[test]
fn the_sketch_grid_is_a_millimetre_with_a_major_line_every_grid_spacing() {
    let mut app = sketching();
    app.scene.settings.grid_spacing = 5.0;
    let grid = app.grid_plane().expect("the sketch has its own grid");
    assert_eq!((grid.spacing, grid.major_every), (1.0, 5));
    // Zoomed out the levels step by that count rather than by ten.
    app.scene.camera.distance = 5000.0;
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let (fine, coarse, _) = crate::render::grid_levels_on(&view, &grid);
    assert!((coarse / fine - 5.0).abs() < 1e-9, "levels {fine} and {coarse}");
    app.close_sketch_tool();
    assert_ne!(app.grid_plane().map(|grid| grid.spacing), Some(1.0));
}

#[test]
fn the_editor_takes_the_right_dock_and_floats_only_with_the_docks_hidden() {
    let mut app = sketching();
    draw(&mut app, Draw::Line, &[[0.0, 0.0], [10.0, 0.0]]);
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.open_inspect(Pick::Curve(tool.sketch.curves[0].id), egui::pos2(400.0, 300.0));
    assert!(super::dock::in_dock(&app));
    draw_one_frame(&mut app);
    assert!(!app.popups.contains_key(KEY), "the floating window was drawn beside the dock");
    assert!(app.popups.contains_key(super::inspect::INSPECT_KEY), "the line's popup was not drawn");
    app.settings.layout.docks_hidden = true;
    draw_one_frame(&mut app);
    assert!(app.popups.contains_key(KEY), "with the docks hidden the window did not float");
}

#[test]
fn letters_pick_tools_and_esc_steps_back_to_selecting() {
    let mut app = sketching();
    let press = |app: &mut App, key: egui::Key| {
        let ctx = egui::Context::default();
        let event = egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let input = egui::RawInput { events: vec![event], ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            app.sketch_keys(ctx);
        });
    };
    press(&mut app, egui::Key::T);
    assert_eq!(app.sketch_tool.as_ref().unwrap().draw, Draw::Trim);
    press(&mut app, egui::Key::Escape);
    assert_eq!(app.sketch_tool.as_ref().unwrap().draw, Draw::Select);
}
