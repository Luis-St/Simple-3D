//! The measure tool: placing a span and reading it.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_measurement_reads_distance_delta_and_the_direction_it_points() {
    // A 3-4-0 span: five long, level, mostly along +Y.
    let m = Measurement::between(Vec3::ZERO, Vec3::new(3.0, 4.0, 0.0));
    assert!((m.distance - 5.0).abs() < 1e-9);
    assert_eq!(m.delta, Vec3::new(3.0, 4.0, 0.0));
    assert!(m.inclination_deg.abs() < 1e-9, "a level span should not be inclined: {}", m.inclination_deg);
    assert!((m.bearing_deg - 53.13010).abs() < 1e-3, "bearing was {}", m.bearing_deg);

    // Straight up: 90 degrees of incline, no meaningful bearing.
    let up = Measurement::between(Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0));
    assert!((up.inclination_deg - 90.0).abs() < 1e-9);
    assert_eq!(up.bearing_deg, 0.0);

    // A zero-length span reads as level, never NaN.
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

    // A third point starts a fresh measurement.
    measure.add(MeasurePoint { at: Vec3::new(5.0, 5.0, 0.0), kind: None });
    assert_eq!(measure.points.len(), 1);
    assert!(measure.span().is_none());
}

#[test]
pub(crate) fn the_measure_tool_snaps_a_click_to_a_bodys_vertex() {
    // A click near a box corner reports the corner exactly.
    let mut app = headless_app();
    let root = app.scene.root();
    let id = app.scene.add_primitive("box", root, 0).unwrap();
    app.scene.get_mut(id).unwrap().position = Vec3::new(0.0, 0.0, 0.0);
    app.reevaluate_for_test();

    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    // Aim a hair off the default 20 mm box's +X +Y +Z corner.
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
    // Issue 78: aiming part-way along an edge used to fall through to the surface.
    let mut app = headless_app();
    let root = app.scene.root();
    let id = app.scene.add_primitive("box", root, 0).unwrap();
    app.reevaluate_for_test();

    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let (lo, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();
    // A third along the top +Y edge, near no corner or midpoint, so only the edge answers.
    let (a, b) = (Vec3::new(lo.x, hi.y, hi.z), Vec3::new(hi.x, hi.y, hi.z));
    let along = a + (b - a) * (1.0 / 3.0);
    let (screen, _) = view.project(along).unwrap();
    let point = app.measure_point_at(&view, screen).expect("a point under the cursor");
    assert_eq!(point.kind, Some(crate::snap::FeatureKind::Edge), "caught {:?}, not the edge", point.kind);
    assert!((point.at - along).length() < 0.2, "caught {:?}, a third along is {:?}", point.at, along);

    // A corner in reach still wins over the edge beside it.
    let (screen, _) = view.project(b).unwrap();
    let point = app.measure_point_at(&view, screen + egui::vec2(2.0, 2.0)).unwrap();
    assert_eq!(point.kind, Some(crate::snap::FeatureKind::Vertex));
}

#[test]
pub(crate) fn either_end_of_a_span_can_be_typed_rather_than_clicked() {
    // Issue 78: the panel's start and end fields write here.
    let mut measure = Measure::default();
    // The end cannot be placed before the start.
    measure.set_point(1, Vec3::new(5.0, 0.0, 0.0));
    assert!(measure.points.is_empty());

    measure.set_point(0, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(measure.points.len(), 1);
    measure.set_point(1, Vec3::new(4.0, 2.0, 3.0));
    let (a, b) = measure.span().expect("both ends are down");
    assert_eq!(a.at, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(b.at, Vec3::new(4.0, 2.0, 3.0));

    // A typed coordinate drops the feature an end had caught.
    measure.points[0].kind = Some(crate::snap::FeatureKind::Vertex);
    measure.set_point(0, Vec3::ZERO);
    assert_eq!(measure.points[0].kind, None);
    let distance = Measurement::between(measure.points[0].at, measure.points[1].at).distance;
    assert!((distance - 29.0_f64.sqrt()).abs() < 1e-9, "the span reads {distance} from the typed ends");
}

#[test]
pub(crate) fn a_placed_end_can_be_taken_back_off_one_at_a_time() {
    // A viewport right-click removes the last placement, down to none.
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
    // With nothing placed it is harmless.
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
    // With only one end down, where the live preview line is drawn.
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
