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

fn count(tool: &SketchTool, kind: &str) -> usize {
    tool.sketch.curves.iter().filter(|c| !c.construction && c.shape.label() == kind).count()
}

fn regions(tool: &SketchTool) -> usize {
    simple3d_sketch::regions::find(&tool.sketch, 64).list.len()
}

#[test]
fn every_tool_is_in_one_family_and_no_two_families_share_a_key() {
    let all: Vec<Draw> = Draw::families().flat_map(|f| f.iter().copied()).collect();
    for draw in &all {
        assert_eq!(all.iter().filter(|d| *d == draw).count(), 1, "{draw:?} is in more than one family");
        assert!(draw.family().contains(draw));
    }
    let keys: Vec<egui::Key> = Draw::keyed().map(|(_, key)| key).collect();
    for key in &keys {
        assert_eq!(keys.iter().filter(|k| *k == key).count(), 1, "{key:?} picks two families");
        assert_ne!(*key, super::keys::CONSTRUCTION_KEY);
    }
}

#[test]
fn a_family_key_steps_through_its_variants_and_a_tile_remembers_the_last() {
    let family = Draw::Rectangle.family();
    assert_eq!(super::keys::next_variant(Draw::Line, Draw::Rectangle, family), Draw::Rectangle);
    assert_eq!(super::keys::next_variant(Draw::Rectangle, Draw::Rectangle, family), Draw::CenterRectangle);
    assert_eq!(super::keys::next_variant(Draw::RoundedRectangle, Draw::Rectangle, family), Draw::Rectangle);
    let mut app = sketching();
    let tool = draw(&mut app, Draw::ThreePointRectangle, &[]);
    tool.set_draw(Draw::Line);
    assert_eq!(tool.settings.variant(family), Draw::ThreePointRectangle);
}

#[test]
fn a_centre_line_runs_as_far_either_side_of_its_middle() {
    let mut app = sketching();
    let tool = draw(&mut app, Draw::CentreLine, &[[1.0, 1.0], [5.0, 3.0]]);
    let Shape::Line { a, b } = tool.sketch.curves[0].shape else { panic!("no line") };
    assert!(near(tool.sketch.at(a), [-3.0, -1.0]) && near(tool.sketch.at(b), [5.0, 3.0]));
    assert!(tool.sketch.constraints.iter().any(|c| matches!(c.kind, ConstraintKind::Midpoint { .. })));
}

#[test]
fn a_three_point_rectangle_stays_square_when_a_corner_is_dragged() {
    let mut app = sketching();
    let tool = draw(&mut app, Draw::ThreePointRectangle, &[[0.0, 0.0], [8.0, 6.0], [0.0, 10.0]]);
    assert_eq!(count(tool, "line"), 4);
    let far = tool.sketch.points.iter().find(|p| near(p.at, [8.0, 6.0])).unwrap().id;
    assert!(tool.sketch.drag(&[(far, [9.0, 4.0])]));
    let corners: Vec<[f64; 2]> = tool.sketch.points.iter().map(|p| p.at).collect();
    for i in 0..4 {
        let (p, q, r) = (corners[i], corners[(i + 1) % 4], corners[(i + 2) % 4]);
        let dot = (q[0] - p[0]) * (r[0] - q[0]) + (q[1] - p[1]) * (r[1] - q[1]);
        assert!(dot.abs() < 1e-5, "corner {i} is no longer square: {corners:?}");
    }
    assert!(tool.analysis.redundant.is_empty());
}

#[test]
fn a_rounded_rectangle_has_four_equal_corner_arcs_and_one_radius() {
    let mut app = sketching();
    app.sketch_tool.as_mut().unwrap().settings.fillet_radius = 2.0;
    let tool = draw(&mut app, Draw::RoundedRectangle, &[[0.0, 0.0], [20.0, 10.0]]);
    assert_eq!((count(tool, "line"), count(tool, "arc")), (4, 4));
    let radii = tool.sketch.constraints.iter().filter(|c| matches!(c.kind, ConstraintKind::Radius { .. })).count();
    let equal = tool.sketch.constraints.iter().filter(|c| matches!(c.kind, ConstraintKind::Equal { .. })).count();
    assert_eq!((radii, equal), (1, 3));
    assert_eq!(regions(tool), 1);
    assert!(tool.analysis.redundant.is_empty() && tool.analysis.solved);
}

#[test]
fn a_two_point_circle_spans_its_clicks_and_an_outer_polygon_touches_its_side_click() {
    let mut app = sketching();
    let tool = draw(&mut app, Draw::TwoPointCircle, &[[0.0, 0.0], [10.0, 0.0]]);
    let Shape::Circle { center, radius } = tool.sketch.curves[0].shape else { panic!("no circle") };
    assert!(near(tool.sketch.at(center), [5.0, 0.0]) && (radius - 5.0).abs() < 1e-9);
    let mut app = sketching();
    app.sketch_tool.as_mut().unwrap().settings.polygon_sides = 6;
    let tool = draw(&mut app, Draw::CircumscribedPolygon, &[[0.0, 0.0], [10.0, 0.0]]);
    let corner = 10.0 / (std::f64::consts::PI / 6.0).cos();
    let corners: Vec<f64> = tool.sketch.points.iter().skip(1).map(|p| p.at[0].hypot(p.at[1])).collect();
    assert!(corners.iter().all(|r| (r - corner).abs() < 1e-6), "corners at {corners:?}");
}

#[test]
fn a_tangent_arc_runs_on_from_a_line_and_the_next_from_the_arc() {
    let mut app = sketching();
    draw(&mut app, Draw::Line, &[[0.0, 0.0], [10.0, 0.0]]);
    let tool = draw(&mut app, Draw::TangentArc, &[[10.0, 0.0], [10.0, 10.0], [5.0, 15.0]]);
    let arcs: Vec<_> = tool
        .sketch
        .curves
        .iter()
        .filter_map(|c| match c.shape {
            Shape::Arc { center, .. } => Some(tool.sketch.at(center)),
            _ => None,
        })
        .collect();
    assert_eq!(arcs.len(), 2, "both clicks after the first make an arc");
    assert!(near(arcs[0], [10.0, 5.0]), "a left turn up from the line's end: {arcs:?}");
    let tangents = tool.sketch.constraints.iter().filter(|c| matches!(c.kind, ConstraintKind::Tangent { .. })).count();
    assert_eq!(tangents, 2);
    assert!(tool.analysis.solved && tool.analysis.redundant.is_empty());
}

#[test]
fn a_tangent_arc_needs_to_start_on_the_end_of_a_line_or_arc() {
    let mut app = sketching();
    let tool = draw(&mut app, Draw::TangentArc, &[[0.0, 0.0], [5.0, 5.0]]);
    assert!(tool.sketch.curves.is_empty());
}

#[test]
fn the_slots_are_closed_and_held_by_just_enough_constraints() {
    for (variant, clicks, free) in [
        (Draw::Slot, vec![[0.0, 0.0], [10.0, 0.0], [5.0, 3.0]], 5),
        (Draw::OverallSlot, vec![[0.0, 0.0], [20.0, 0.0], [5.0, 3.0]], 5),
        (Draw::ArcSlot, vec![[0.0, 0.0], [20.0, 0.0], [0.0, 20.0], [0.0, 23.0]], 6),
    ] {
        let mut app = sketching();
        let tool = draw(&mut app, variant, &clicks);
        assert_eq!(regions(tool), 1, "{variant:?} is not one closed region");
        assert!(tool.analysis.solved, "{variant:?} does not solve");
        assert!(tool.analysis.redundant.is_empty(), "{variant:?} repeats a constraint");
        assert_eq!(tool.analysis.dof, free, "{variant:?} is held wrongly");
    }
    let mut app = sketching();
    let tool = draw(&mut app, Draw::OverallSlot, &[[0.0, 0.0], [20.0, 0.0], [5.0, 3.0]]);
    let centres: Vec<[f64; 2]> = tool
        .sketch
        .curves
        .iter()
        .filter_map(|c| match c.shape {
            Shape::Arc { center, .. } => Some(tool.sketch.at(center)),
            _ => None,
        })
        .collect();
    assert!(centres.iter().any(|&c| near(c, [3.0, 0.0])) && centres.iter().any(|&c| near(c, [17.0, 0.0])));
}

#[test]
fn an_ellipse_takes_its_width_from_the_third_click_and_bounds_a_region() {
    let mut app = sketching();
    let tool = draw(&mut app, Draw::Ellipse, &[[0.0, 0.0], [10.0, 0.0], [3.0, 4.0]]);
    let Some(Shape::Ellipse { minor, .. }) =
        tool.sketch.curves.iter().map(|c| c.shape.clone()).find(|s| s.is_elliptic())
    else {
        panic!("no ellipse")
    };
    assert!((minor - 4.0).abs() < 1e-9);
    assert_eq!(regions(tool), 1);
    let mut app = sketching();
    let tool = draw(&mut app, Draw::EllipseArc, &[[0.0, 0.0], [10.0, 0.0], [6.0, 4.0], [-10.0, 1.0]]);
    assert_eq!(count(tool, "elliptical arc"), 1);
    assert!(tool.analysis.solved);
}

#[test]
fn with_helper_geometry_switched_on_everything_drawn_is_helper_geometry() {
    let mut app = sketching();
    app.sketch_tool.as_mut().unwrap().settings.construction = true;
    let tool = draw(&mut app, Draw::Slot, &[[0.0, 0.0], [10.0, 0.0], [5.0, 3.0]]);
    assert!(!tool.sketch.curves.is_empty() && tool.sketch.curves.iter().all(|c| c.construction));
    assert_eq!(regions(tool), 0);
}
