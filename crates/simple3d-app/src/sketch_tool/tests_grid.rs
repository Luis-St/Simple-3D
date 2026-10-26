use super::pointer::snapped_delta;
use super::*;
use crate::app::tests::headless_app;
use crate::app::App;
use crate::render::GridPlane;

fn sketching_on(frame: Xform) -> App {
    let mut app = headless_app();
    app.selection.clear();
    app.toggle_sketch_tool();
    app.place_sketch(frame, None);
    app
}

#[test]
fn the_axis_out_of_the_sketch_plane_is_hidden_while_sketching() {
    let mut app = headless_app();
    app.scene.settings.axes_visible = [true; 3];
    assert_eq!(app.shown_axes(), [true; 3]);
    let mut app = sketching_on(Xform::from_translation(Vec3::ZERO));
    app.scene.settings.axes_visible = [true; 3];
    assert_eq!(app.shown_axes(), [true, true, false], "the ground's Z axis still showed");
    // On the XZ plane it is Y that points at the eye.
    let xz = plane_frame(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0));
    let mut app = sketching_on(xz);
    app.scene.settings.axes_visible = [true; 3];
    assert_eq!(app.shown_axes(), [true, false, true]);
    // A tilted plane has no axis straight out of it.
    let tilted = plane_frame(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 1.0).normalized());
    let mut app = sketching_on(tilted);
    app.scene.settings.axes_visible = [true; 3];
    assert_eq!(app.shown_axes(), [true; 3]);
    app.close_sketch_tool();
    assert_eq!(app.shown_axes(), [true; 3]);
}

#[test]
fn the_grid_spacing_lines_are_heavy_only_against_the_millimetre_lines() {
    let mut app = sketching_on(Xform::from_translation(Vec3::ZERO));
    app.scene.settings.grid_spacing = 10.0;
    let grid = app.grid_plane().expect("the sketch has its own grid");
    // Zoomed right in, the millimetre level is the coarse one and its majors are the heavy lines.
    assert_eq!(grid.bold(1.0, 1.0, 0.0), (1.0, false));
    // Over the fading millimetre level every 10 mm line is heavy, fully once the fine level shows.
    assert_eq!(grid.bold(1.0, 10.0, 0.1), (0.4, true));
    assert_eq!(grid.bold(1.0, 10.0, 0.4), (1.0, true));
    // Zoomed out past it there is nothing finer to stand out against.
    assert_eq!(grid.bold(10.0, 100.0, 1.0), (0.0, false));
    assert_eq!(GridPlane::ground(10.0).bold(1.0, 10.0, 1.0), (0.0, false), "the ground grid changed");
}

#[test]
fn clicks_snap_to_the_grid_without_a_key_and_not_while_it_is_hidden() {
    let mut app = sketching_on(Xform::from_translation(Vec3::ZERO));
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let step = super::grid_step(&app, &view).expect("the grid is shown");
    assert!(!app.snap_requested && step > 0.0);
    app.scene.settings.grid_visible = false;
    assert_eq!(super::grid_step(&app, &view), None);
}

#[test]
fn a_drag_puts_the_point_it_began_at_onto_the_grid() {
    let drag =
        Dragging { start: [0.3, 0.2], points: vec![(0, [0.25, 0.25]), (1, [5.25, 0.25])], before: Sketch::default() };
    let delta = snapped_delta(&drag, [2.6, 1.1], Some(1.0));
    let moved = [0.25 + delta[0], 0.25 + delta[1]];
    assert!((moved[0] - 3.0).abs() < 1e-9 && (moved[1] - 1.0).abs() < 1e-9, "landed at {moved:?}");
    assert_eq!(snapped_delta(&drag, [2.6, 1.1], None), [2.6, 1.1]);
}

#[test]
fn a_click_snapped_onto_a_point_out_of_reach_reuses_it() {
    // Closing a triangle a little off its first corner snapped onto the same grid spot but made a
    // second point there, so the corner came apart as soon as a constraint moved it.
    let mut app = sketching_on(Xform::from_translation(Vec3::ZERO));
    let view = crate::view::View::new(app.scene.camera, app.viewport_rect);
    let tool = app.sketch_tool.as_mut().unwrap();
    tool.set_draw(Draw::Point);
    tool.click(Click { at: [10.0, 0.0], point: None });
    let corner = tool.sketch.points[0].id;
    let cursor = tool.screen(&view, [14.0, 3.0]).unwrap();
    assert!(tool.point_at(&view, cursor, &[]).is_none(), "the cursor must be out of the point's reach");
    let click = tool.click_at(&view, cursor, Some(10.0)).unwrap();
    assert_eq!(click.at, [10.0, 0.0]);
    assert_eq!(click.point, Some(corner));
}
