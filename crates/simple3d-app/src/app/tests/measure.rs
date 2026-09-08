//! The measure tool: placing a span and reading it.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_measurement_reads_distance_delta_and_the_direction_it_points() {
    // A 3-4-0 span: five long, level, and running mostly along +Y from +X.
    let m = Measurement::between(Vec3::ZERO, Vec3::new(3.0, 4.0, 0.0));
    assert!((m.distance - 5.0).abs() < 1e-9);
    assert_eq!(m.delta, Vec3::new(3.0, 4.0, 0.0));
    assert!(m.inclination_deg.abs() < 1e-9, "a level span should not be inclined: {}", m.inclination_deg);
    assert!((m.bearing_deg - 53.13010).abs() < 1e-3, "bearing was {}", m.bearing_deg);

    // Straight up: ninety degrees of incline, and no bearing to speak of.
    let up = Measurement::between(Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0));
    assert!((up.inclination_deg - 90.0).abs() < 1e-9);
    assert_eq!(up.bearing_deg, 0.0);

    // A zero-length span is level rather than undefined, so the readout never
    // shows NaN while a second point is being aimed.
    let none = Measurement::between(Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(none.distance, 0.0);
    assert_eq!(none.inclination_deg, 0.0);
}

#[test]
pub(crate) fn the_measure_tool_takes_two_points_and_the_third_begins_a_new_span() {
    let mut measure = Measure::default();
    assert!(measure.span().is_none());
    measure.add(MeasurePoint { at: Vec3::ZERO, kind: None });
    assert!(measure.span().is_none(), "one point is not a span");
    measure.add(MeasurePoint { at: Vec3::new(10.0, 0.0, 0.0), kind: Some(crate::snap::FeatureKind::Vertex) });
    assert!(measure.span().is_some(), "two points make a span");

    // A third point starts fresh rather than piling up, so the tool flows
    // from one measurement to the next.
    measure.add(MeasurePoint { at: Vec3::new(5.0, 5.0, 0.0), kind: None });
    assert_eq!(measure.points.len(), 1);
    assert!(measure.span().is_none());
}

#[test]
pub(crate) fn the_measure_tool_snaps_a_click_to_a_bodys_vertex() {
    // The whole point of picking features rather than raw surface hits: a
    // click near a box corner reports the corner exactly.
    let mut app = headless_app();
    let root = app.scene.root();
    let id = app.scene.add_primitive("box", root, 0).unwrap();
    app.scene.get_mut(id).unwrap().position = Vec3::new(0.0, 0.0, 0.0);
    app.reevaluate_for_test();

    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    // A box is 20mm to a side by default; aim a hair off its +X +Y +Z corner.
    let (lo, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();
    let corner = Vec3::new(hi.x, hi.y, hi.z);
    let (screen, _) = view.project(corner).unwrap();
    let point = app.measure_point_at(&view, screen + egui::vec2(3.0, 3.0)).expect("a point under the cursor");
    assert_eq!(point.kind, Some(crate::snap::FeatureKind::Vertex), "the click did not catch the corner");
    assert!((point.at - corner).length() < 1e-6, "snapped to {:?}, not the corner {:?}", point.at, corner);
    let _ = lo;
}

#[test]
pub(crate) fn a_measure_click_between_two_corners_catches_the_edge_itself() {
    // Issue 78: aiming at the middle of nothing in particular, part-way along
    // an edge, used to fall through to the surface hit under the pointer.
    let mut app = headless_app();
    let root = app.scene.root();
    let id = app.scene.add_primitive("box", root, 0).unwrap();
    app.reevaluate_for_test();

    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let (lo, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();
    // A third of the way along the top +Y edge: near no corner and not the
    // midpoint either, so only the edge itself can answer.
    let (a, b) = (Vec3::new(lo.x, hi.y, hi.z), Vec3::new(hi.x, hi.y, hi.z));
    let along = a + (b - a) * (1.0 / 3.0);
    let (screen, _) = view.project(along).unwrap();
    let point = app.measure_point_at(&view, screen).expect("a point under the cursor");
    assert_eq!(point.kind, Some(crate::snap::FeatureKind::Edge), "caught {:?}, not the edge", point.kind);
    assert!((point.at - along).length() < 0.2, "caught {:?}, a third along is {:?}", point.at, along);

    // A corner still wins where one is in reach, so aiming at a corner never
    // lands part-way along the edge beside it.
    let (screen, _) = view.project(b).unwrap();
    let point = app.measure_point_at(&view, screen + egui::vec2(2.0, 2.0)).unwrap();
    assert_eq!(point.kind, Some(crate::snap::FeatureKind::Vertex));
}

#[test]
pub(crate) fn either_end_of_a_span_can_be_typed_rather_than_clicked() {
    // Issue 78: the property panel's start and end fields write here.
    let mut measure = Measure::default();
    // Nothing is placed yet, so the end cannot be: it would be a point with
    // nothing to measure to.
    measure.set_point(1, Vec3::new(5.0, 0.0, 0.0));
    assert!(measure.points.is_empty());

    measure.set_point(0, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(measure.points.len(), 1);
    measure.set_point(1, Vec3::new(4.0, 2.0, 3.0));
    let (a, b) = measure.span().expect("both ends are down");
    assert_eq!(a.at, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(b.at, Vec3::new(4.0, 2.0, 3.0));

    // Typing a coordinate over an end that had caught a feature makes it an
    // exact point rather than leaving it claiming a feature it has left.
    measure.points[0].kind = Some(crate::snap::FeatureKind::Vertex);
    measure.set_point(0, Vec3::ZERO);
    assert_eq!(measure.points[0].kind, None);
    let distance = Measurement::between(measure.points[0].at, measure.points[1].at).distance;
    assert!((distance - 29.0_f64.sqrt()).abs() < 1e-9, "the span reads {distance} from the typed ends");
}

#[test]
pub(crate) fn a_placed_end_can_be_taken_back_off_one_at_a_time() {
    // A right-click in the viewport undoes the last placement, back to
    // nothing placed at all.
    let mut app = headless_app();
    app.toggle_measure();
    app.measure_click(MeasurePoint { at: Vec3::ZERO, kind: None });
    app.measure_click(MeasurePoint { at: Vec3::new(10.0, 0.0, 0.0), kind: None });
    assert!(app.measure.span().is_some());

    app.measure_unplace();
    assert_eq!(app.measure.points.len(), 1, "the end did not come off");
    assert!(app.measure.span().is_none());
    app.measure_unplace();
    assert!(app.measure.points.is_empty(), "the start did not come off");
    // And with nothing placed it is harmless.
    app.measure_unplace();
    assert!(app.measure.points.is_empty());
    assert!(app.measure.active, "taking a point back also put the tool away");
}

#[test]
pub(crate) fn the_measure_overlay_draws_a_placed_span_without_panicking() {
    let mut app = headless_app();
    app.measure.active = true;
    app.measure.add(MeasurePoint { at: Vec3::ZERO, kind: Some(crate::snap::FeatureKind::Vertex) });
    app.measure.add(MeasurePoint { at: Vec3::new(20.0, 8.0, 5.0), kind: Some(crate::snap::FeatureKind::FaceCentre) });
    draw_one_frame(&mut app);
    // And with only one end down, where the live preview line is drawn.
    app.measure.clear();
    app.measure.add(MeasurePoint { at: Vec3::ZERO, kind: None });
    draw_one_frame(&mut app);
}

#[test]
pub(crate) fn picking_a_transform_tool_puts_the_measure_tool_away() {
    let mut app = headless_app();
    app.run(Command::MeasureTool);
    assert!(app.measure.active);
    app.measure.add(MeasurePoint { at: Vec3::ZERO, kind: None });
    app.run(Command::ModeRotate);
    assert!(!app.measure.active, "the measure tool held the pointer after a transform tool was chosen");
    assert!(app.measure.points.is_empty(), "its span was left hanging in the scene");
}
