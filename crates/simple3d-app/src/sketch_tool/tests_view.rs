use super::origin::OriginPlane;
use super::*;
use crate::app::tests::headless_app;
use crate::app::App;
use crate::view::{View, ViewPreset};

fn sketching_on(frame: Xform) -> App {
    let mut app = headless_app();
    app.settings.reduce_motion = true;
    app.selection.clear();
    app.toggle_sketch_tool();
    app.place_sketch(frame, None);
    app
}

fn near(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-6
}

/// Whether the view shows `frame`'s X to the right and its Y up.
fn square_on(app: &App, frame: Xform) -> bool {
    let (right, up) = View::new(app.scene.camera, app.viewport_rect).basis();
    near(right, frame.axis(0)) && near(up, frame.axis(1))
}

#[test]
fn looking_at_a_sketch_puts_its_x_right_and_its_y_up() {
    for plane in OriginPlane::ALL {
        let app = sketching_on(plane.frame());
        assert!(square_on(&app, plane.frame()), "{plane:?} was not square on");
    }
    // A sloping face, steep enough that its old axes had X running uphill.
    for normal in [Vec3::new(0.0, -0.3, 0.95), Vec3::new(0.6, 0.2, -0.5), Vec3::new(0.0, 0.0, -1.0)] {
        let (u, v) = pointer::sketch_axes(normal);
        assert!(near(u.cross(v), normal.normalized()), "the axes of {normal:?} do not face it");
        let frame = plane_frame(Vec3::ZERO, u, v);
        let app = sketching_on(frame);
        assert!(square_on(&app, frame), "a face toward {normal:?} was not square on");
    }
}

#[test]
fn the_2d_lock_holds_views_back_until_it_is_turned_off() {
    let frame = OriginPlane::XZ.frame();
    let mut app = sketching_on(frame);
    assert!(app.sketch_flat(), "the lock is not on by default");
    app.set_view(ViewPreset::Top);
    assert!(square_on(&app, frame), "a view key turned away from the sketch");
    app.toggle_sketch_lock();
    assert!(!app.sketch_flat());
    app.set_view(ViewPreset::Top);
    assert!(!square_on(&app, frame), "with the lock off the view key did nothing");
    // Turning the lock back on turns back onto the sketch.
    app.toggle_sketch_lock();
    assert!(app.sketch_flat() && square_on(&app, frame));
    app.close_sketch_tool();
    assert!(!app.sketch_flat());
}

#[test]
fn turning_back_onto_the_sketch_keeps_where_the_view_was_panned_to() {
    let frame = OriginPlane::XY.frame();
    let mut app = sketching_on(frame);
    assert!(near(app.scene.camera.target, Vec3::ZERO), "a new sketch did not centre on its plane");
    app.toggle_sketch_lock();
    app.scene.camera.target = Vec3::new(40.0, -25.0, 12.0);
    app.set_view(ViewPreset::Front);
    app.look_at_sketch();
    assert!(square_on(&app, frame));
    assert!(near(app.scene.camera.target, Vec3::new(40.0, -25.0, 0.0)), "the pan was lost");
}

#[test]
fn looking_at_the_sketch_returns_to_its_origin() {
    let frame = OriginPlane::XZ.frame();
    let mut app = sketching_on(frame);
    app.toggle_sketch_lock();
    app.scene.camera.target = Vec3::new(40.0, -25.0, 12.0);
    app.set_view(ViewPreset::Top);
    app.return_to_sketch();
    assert!(square_on(&app, frame));
    assert!(near(app.scene.camera.target, frame.t), "the view did not return to the sketch's origin");
}
