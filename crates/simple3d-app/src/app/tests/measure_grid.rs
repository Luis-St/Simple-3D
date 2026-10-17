//! Position picks snapping to the ground grid, and the one pick path's other options (issue 126).

use super::*;
use crate::snap::FeatureKind;
use simple3d_geom::Vec3;

/// An empty scene, its view, and the grid spacing a pick snaps to there.
fn empty_ground() -> (App, crate::view::View, f64) {
    let app = headless_app();
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let spacing = app.ground_grid(&view).expect("the grid is shown by default").spacing;
    (app, view, spacing)
}

#[test]
pub(crate) fn a_snapping_pick_lands_exactly_on_a_grid_crossing_near_the_pointer() {
    let (app, view, s) = empty_ground();
    let crossing = Vec3::new(2.0 * s, 3.0 * s, 0.0);
    let (screen, _) = view.project(crossing).unwrap();
    let point = snapped_pick(&app, &view, screen + egui::vec2(3.0, -2.0)).expect("the ground is under the pointer");
    assert_eq!(point.kind, Some(FeatureKind::GridPoint), "caught {:?}", point.kind);
    assert!((point.at - crossing).length() < 1e-9, "landed at {:?}, not on the crossing", point.at);
}

#[test]
pub(crate) fn between_crossings_a_snapping_pick_lands_on_the_grid_line() {
    let (app, view, s) = empty_ground();
    let (a, b) = (Vec3::new(2.0 * s, 2.0 * s, 0.0), Vec3::new(2.0 * s, 3.0 * s, 0.0));
    let half_cell = (view.project(a).unwrap().0 - view.project(b).unwrap().0).length() / 2.0;
    assert!(half_cell > crate::snap::CATCH_PIXELS, "cells too small to point between crossings: {half_cell} px");
    let (screen, _) = view.project((a + b) * 0.5).unwrap();
    let point = snapped_pick(&app, &view, screen + egui::vec2(2.0, 0.0)).unwrap();
    assert_eq!(point.kind, Some(FeatureKind::GridLine), "caught {:?}", point.kind);
    assert!((point.at.x - 2.0 * s).abs() < 1e-9 && point.at.z.abs() < 1e-9, "{:?} is off the line", point.at);
}

#[test]
pub(crate) fn on_an_axis_a_grid_point_wins_over_the_axis_itself() {
    // The axis is caught anywhere along it, but near one of its grid points the point is meant.
    let (app, view, s) = empty_ground();
    let crossing = Vec3::new(3.0 * s, 0.0, 0.0);
    let (screen, _) = view.project(crossing).unwrap();
    let point = snapped_pick(&app, &view, screen + egui::vec2(2.0, 1.0)).unwrap();
    assert_eq!(point.kind, Some(FeatureKind::GridPoint), "caught {:?}", point.kind);
    assert!((point.at - crossing).length() < 1e-9, "landed at {:?}", point.at);
}

#[test]
pub(crate) fn without_snapping_asked_for_a_pick_lands_where_the_pointer_is() {
    let (app, view, s) = empty_ground();
    let crossing = Vec3::new(2.0 * s, 3.0 * s, 0.0);
    let (screen, _) = view.project(crossing).unwrap();
    let (point, mark) = app.pick_position(&view, screen + egui::vec2(3.0, -2.0), &[]).unwrap();
    assert_eq!(point.kind, None, "snapped to {:?} with snapping not asked for", point.kind);
    assert!(mark.is_none());
    assert!((point.at - crossing).length() > 1e-6, "landed on the crossing anyway");
}

#[test]
pub(crate) fn a_hidden_grid_is_not_snapped_to() {
    let (mut app, view, s) = empty_ground();
    app.scene.settings.grid_visible = false;
    let (screen, _) = view.project(Vec3::new(2.0 * s, 3.0 * s, 0.0)).unwrap();
    let point = snapped_pick(&app, &view, screen + egui::vec2(3.0, -2.0)).unwrap();
    assert_eq!(point.kind, None, "caught {:?} on a hidden grid", point.kind);
}

#[test]
pub(crate) fn a_pick_leaving_a_body_out_does_not_catch_its_corners() {
    // A shape being put down covers what it is aimed at, so it must not catch itself.
    let mut app = headless_app();
    let root = app.scene.root();
    let id = app.scene.add_primitive("box", root, 0).unwrap();
    app.scene.get_mut(id).unwrap().position = Vec3::ZERO;
    app.reevaluate_for_test();
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let (_, hi) = app.evaluated.node_meshes[&id].bounds().unwrap();
    let (screen, _) = view.project(hi).unwrap();
    let cursor = screen + egui::vec2(3.0, 3.0);
    let caught = app.snap_catch(&view, cursor, &[]).map(|(point, _)| point.kind);
    assert_eq!(caught, Some(Some(FeatureKind::Vertex)), "the corner itself is not caught");
    let left_out = app.snap_catch(&view, cursor, &[id]).map(|(point, _)| point.kind);
    assert_ne!(left_out, Some(Some(FeatureKind::Vertex)), "the left-out box's corner was caught");
}

#[test]
pub(crate) fn rounding_onto_the_grid_keeps_a_point_on_its_surface() {
    let at = Vec3::new(1.4, 2.6, 3.3);
    let up = Vec3::new(0.0, 0.0, 1.0);
    assert_eq!(crate::snap::on_grid(at, Some(up), 1.0), Vec3::new(1.0, 3.0, 3.3), "a top face keeps its height");
    assert_eq!(crate::snap::on_grid(at, None, 1.0), Vec3::new(1.0, 3.0, 3.0), "with no surface every axis rounds");
    let slanted = Vec3::new(0.0, 1.0, 1.0).normalized();
    assert_eq!(crate::snap::on_grid(at, Some(slanted), 1.0), at, "a slanted surface has no grid");
    assert_eq!(crate::snap::on_grid(at, Some(up), 0.0), at, "no step, no rounding");
}
