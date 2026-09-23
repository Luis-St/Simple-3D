use super::grips::*;
use super::window::*;
use crate::panel_properties::component;
use simple3d_core::scene::SectionView;
use simple3d_geom::Vec3;

use super::*;

fn section(axis: usize, offset: f64) -> SectionView {
    SectionView { enabled: true, axis, offset, ..SectionView::default() }
}

#[test]
fn the_frame_lies_in_the_plane_and_covers_the_model() {
    let bounds = Some((Vec3::new(-10.0, -20.0, 0.0), Vec3::new(10.0, 20.0, 30.0)));
    let corners = frame(&section(2, 12.0), bounds);
    assert!(corners.iter().all(|c| (c.z - 12.0).abs() < 1e-9), "a corner left the plane: {corners:?}");
    // Wider than the model in both of the directions it spans, so the plane
    // reads as passing through the shape.
    let reach = corners.iter().fold(Vec3::splat(0.0), |m, c| m.max(Vec3::new(c.x.abs(), c.y.abs(), 0.0)));
    assert!(reach.x > 10.0 && reach.y > 20.0, "the frame is inside the model: {reach:?}");
}

#[test]
fn the_frame_follows_a_model_that_is_nowhere_near_the_origin() {
    let bounds = Some((Vec3::new(300.0, 300.0, 0.0), Vec3::new(320.0, 320.0, 10.0)));
    let corners = frame(&section(2, 5.0), bounds);
    let middle = (corners[0] + corners[2]) * 0.5;
    assert!((middle.x - 310.0).abs() < 1e-9 && (middle.y - 310.0).abs() < 1e-9, "the frame is centred at {middle:?}");
}

#[test]
fn the_plane_is_held_at_each_edge_and_in_the_middle() {
    // Five places rather than one, so which of them the model is hiding
    // depends on where it has been orbited to and never on all of them at
    // once (issue 72).
    let bounds = Some((Vec3::new(-10.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 20.0)));
    for axis in 0..3 {
        let corners = frame(&section(axis, 5.0), bounds);
        let middle = (corners[0] + corners[2]) * 0.5;
        let held = grips(&corners);

        // The last one is the middle of the plane, and the four before it
        // are the middles of its edges -- each one on the line between two
        // corners, and none of them a corner.
        assert!((held[4] - middle).length() < 1e-9, "the fifth grip is not the middle of the plane on axis {axis}");
        for (index, &at) in held[..4].iter().enumerate() {
            let (from, to) = (corners[index], corners[(index + 1) % 4]);
            assert!((at - (from + to) * 0.5).length() < 1e-9, "grip {index} is not on the middle of its edge");
            assert!((at - from).length() > 1e-6 && (at - to).length() > 1e-6, "grip {index} landed on a corner");
        }

        // And each of the four stands off the middle, so the one that is
        // over the model is never the only one there is.
        assert!(
            held[..4].iter().all(|&at| (at - middle).length() > 10.0),
            "an edge grip sits on top of the model on axis {axis}"
        );

        // Every one of them is in the plane: a grip that slides the cut has
        // to be where the cut is, or it would be pointing at the wrong
        // coordinate.
        assert!(
            held.iter().all(|&at| (component(at, axis) - 5.0).abs() < 1e-9),
            "a grip left the plane on axis {axis}"
        );
    }
}

#[test]
fn a_thin_model_still_gets_a_frame_worth_grabbing() {
    let bounds = Some((Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 40.0)));
    let corners = frame(&section(2, 5.0), bounds);
    assert!(corners.iter().any(|c| c.x.abs() >= MIN_HALF), "the frame is too small to take hold of");
}

#[test]
fn the_plane_starts_in_the_middle_of_what_it_is_cutting() {
    let bounds = Some((Vec3::new(0.0, 0.0, 4.0), Vec3::new(10.0, 10.0, 24.0)));
    assert!((middle_of(bounds, 2) - 14.0).abs() < 1e-9);
    // Nothing in the scene: the origin, which is where a first shape lands.
    assert_eq!(middle_of(None, 1), 0.0);
}

#[test]
fn the_side_the_camera_looks_from_is_the_one_cut_away() {
    // Issue 109: nothing to set. Looked at from above, the top goes; orbited
    // round underneath, the bottom goes, and the plane itself stays put.
    let bounds = Some((Vec3::new(-10.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 20.0)));
    let plane = section(2, 5.0);
    let (above, below) = (Vec3::new(0.0, 0.0, 15.0), Vec3::new(0.0, 0.0, -5.0));

    let from_above = plane.plane(bounds, Vec3::new(0.3, 0.2, -1.0)).unwrap();
    assert!(!from_above.keeps(above) && from_above.keeps(below), "looking down, the top was kept");

    let from_below = plane.plane(bounds, Vec3::new(0.3, 0.2, 1.0)).unwrap();
    assert!(from_below.keeps(above) && !from_below.keeps(below), "looking up, the bottom was kept");
    assert!(
        from_above.depth(Vec3::new(0.0, 0.0, 5.0)).abs() < 1e-9
            && from_below.depth(Vec3::new(0.0, 0.0, 5.0)).abs() < 1e-9
    );
}

#[test]
fn a_turned_plane_swings_about_the_model_and_its_frame_stays_in_it() {
    let bounds = Some((Vec3::new(300.0, 300.0, 0.0), Vec3::new(320.0, 320.0, 20.0)));
    let mut turned = section(2, 10.0);
    turned.tilt = [30.0, -20.0, 45.0];
    let plane = turned.plane(bounds, Vec3::new(0.0, 0.0, -1.0)).unwrap();

    // Turned about the middle of the model, so it still passes through it
    // rather than swinging off round the origin.
    let middle = Vec3::new(310.0, 310.0, 10.0);
    assert!(plane.depth(middle).abs() < 1e-9, "the turned plane left the model: {}", plane.depth(middle));
    assert!((plane.normal.dot(turned.normal()).abs() - 1.0).abs() < 1e-9);

    // The frame and every grip lie in the plane the renderer cuts with.
    let corners = frame(&turned, bounds);
    for at in corners.iter().chain(grips(&corners).iter()) {
        assert!(plane.depth(*at).abs() < 1e-9, "{at:?} is off the turned plane");
    }

    // Sliding along the normal moves the plane by exactly as far.
    let mut slid = turned;
    slid.offset += 7.0;
    let moved = slid.plane(bounds, Vec3::new(0.0, 0.0, -1.0)).unwrap();
    assert!((moved.depth(middle).abs() - 7.0).abs() < 1e-9, "slid by {}", moved.depth(middle));
}

#[test]
fn an_untilted_plane_stands_at_its_offset_wherever_the_model_is() {
    let bounds = Some((Vec3::new(300.0, 300.0, 0.0), Vec3::new(320.0, 320.0, 20.0)));
    let plane = section(0, 12.0).plane(bounds, Vec3::new(-1.0, 0.0, 0.0)).unwrap();
    assert!(plane.depth(Vec3::new(12.0, 0.0, 0.0)).abs() < 1e-9 && plane.normal.x.abs() > 0.999);
}

#[test]
fn a_plane_clear_of_the_model_cuts_nothing() {
    let corners = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0), Vec3::new(0.0, 10.0, 20.0)];
    let one = [[0, 1, 2]];
    let through = simple3d_geom::section::Plane::new(Vec3::new(0.0, 0.0, 1.0), 5.0);
    assert!(super::crossing::crosses(&corners, &one, &through));
    // Past the top, and past the bottom from the other side: nothing either way.
    for plane in [
        simple3d_geom::section::Plane::new(Vec3::new(0.0, 0.0, 1.0), 25.0),
        simple3d_geom::section::Plane::new(Vec3::new(0.0, 0.0, -1.0), 3.0),
    ] {
        assert!(!super::crossing::crosses(&corners, &one, &plane), "{plane:?} crossed a model it is clear of");
    }
    assert!(!super::crossing::crosses(&[], &[], &through), "an empty scene has nothing to cut");

    // Two bodies with the plane in the gap between them: corners on both
    // sides of it, and still nothing it runs through.
    let apart = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(10.0, 0.0, 0.0),
        Vec3::new(0.0, 10.0, 2.0),
        Vec3::new(0.0, 0.0, 10.0),
        Vec3::new(10.0, 0.0, 10.0),
        Vec3::new(0.0, 10.0, 12.0),
    ];
    assert!(!super::crossing::crosses(&apart, &[[0, 1, 2], [3, 4, 5]], &through), "the gap between two bodies cut");

    // Cut down to a rectangle beside the triangle, the plane misses it.
    let beside = through.within(simple3d_geom::section::Window {
        centre: Vec3::new(50.0, 50.0, 5.0),
        u: Vec3::new(1.0, 0.0, 0.0),
        v: Vec3::new(0.0, 1.0, 0.0),
        half: [5.0, 5.0],
    });
    assert!(!super::crossing::crosses(&corners, &one, &beside), "a rectangle off to the side cut the model");
}

#[test]
fn a_fixed_side_stays_put_however_the_camera_looks() {
    let bounds = Some((Vec3::new(-10.0, -10.0, 0.0), Vec3::new(10.0, 10.0, 20.0)));
    let (above, below) = (Vec3::new(0.0, 0.0, 15.0), Vec3::new(0.0, 0.0, -5.0));
    for (keep, keeps_above) in
        [(simple3d_core::scene::SectionKeep::Below, false), (simple3d_core::scene::SectionKeep::Above, true)]
    {
        let mut plane = section(2, 5.0);
        plane.keep = keep;
        for forward in [Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 0.0, 1.0)] {
            let cut = plane.plane(bounds, forward).unwrap();
            assert_eq!(cut.keeps(above), keeps_above, "{keep:?} looking along {forward:?}");
            assert_eq!(cut.keeps(below), !keeps_above, "{keep:?} looking along {forward:?}");
        }
    }
}

#[test]
fn a_custom_size_cuts_only_behind_its_frame() {
    let bounds = Some((Vec3::new(-50.0, -50.0, 0.0), Vec3::new(50.0, 50.0, 20.0)));
    let mut sized = section(2, 10.0);
    sized.custom_size = true;
    sized.size = [20.0, 10.0];
    let plane = sized.plane(bounds, Vec3::new(0.0, 0.0, -1.0)).unwrap();
    // Looking down, the top goes -- but only straight above the rectangle.
    assert!(!plane.keeps(Vec3::new(0.0, 0.0, 15.0)) && !plane.keeps(Vec3::new(9.0, 4.0, 15.0)));
    assert!(plane.keeps(Vec3::new(11.0, 0.0, 15.0)) && plane.keeps(Vec3::new(0.0, 6.0, 15.0)));
    assert!(plane.keeps(Vec3::new(0.0, 0.0, 5.0)), "below the plane went");
    // And the frame drawn round it is that rectangle.
    let corners = frame(&sized, bounds);
    assert!((corners[0] - Vec3::new(-10.0, -5.0, 10.0)).length() < 1e-9, "{corners:?}");
    assert!((corners[2] - Vec3::new(10.0, 5.0, 10.0)).length() < 1e-9, "{corners:?}");
}

#[test]
fn a_small_window_inside_a_body_still_cuts_it() {
    // A 20 mm box and an 8 mm rectangle through its middle: the surface does
    // not cross the rectangle anywhere, and the rectangle is still inside the
    // material, so there is a pocket to open.
    let mesh = simple3d_geom::primitives::box_mesh(20.0, 20.0, 20.0);
    let window = |x: f64| simple3d_geom::section::Window {
        centre: Vec3::new(x, 0.0, 10.0),
        u: Vec3::new(1.0, 0.0, 0.0),
        v: Vec3::new(0.0, 1.0, 0.0),
        half: [4.0, 4.0],
    };
    let plane = |x: f64| simple3d_geom::section::Plane::new(Vec3::new(0.0, 0.0, 1.0), 10.0).within(window(x));
    assert!(super::crossing::crosses(&mesh.positions, &mesh.indices, &plane(0.0)), "the pocket was not cut");
    assert!(!super::crossing::crosses(&mesh.positions, &mesh.indices, &plane(40.0)), "a rectangle beside the box cut");
}

#[test]
fn several_sections_cut_at_once_and_one_clear_of_the_model_does_not() {
    let mut app = crate::app::tests::headless_app();
    // What the viewport draws, and so what the cut is asked of.
    app.scene_renderable = crate::render::Renderable::prepare_scene(&app.evaluated.mesh, &app.evaluated.ranges);
    app.run(simple3d_core::keymap::Command::ToggleSection);
    let (lo, hi) = app.evaluated.bounds.expect("the plate is in the scene");
    let forward = Vec3::new(0.3, 0.2, -1.0);
    assert_eq!(super::cut(&mut app, forward).len(), 1);

    // A second section across the first, through the model: both cut.
    let second = app.scene.settings.add_section(SectionView {
        enabled: true,
        axis: 1,
        offset: (lo.y + hi.y) * 0.5,
        ..SectionView::default()
    });
    assert_eq!(second, 1);
    assert_eq!(super::cut(&mut app, forward).len(), 2);

    // Slid clear of the model, the second cuts nothing and the first still does.
    app.scene.settings.section_at_mut(1).offset = hi.y + 50.0;
    assert_eq!(super::cut(&mut app, forward).len(), 1);

    // Taking the first away leaves the second in its place, still on.
    app.scene.settings.remove_section(0);
    assert_eq!(app.scene.settings.section_count(), 1);
    assert!(app.scene.settings.section.enabled, "removing the first switched the tool off");
    assert_eq!(app.scene.settings.section.axis, 1);
    // The last one cannot go.
    app.scene.settings.remove_section(0);
    assert_eq!(app.scene.settings.section_count(), 1);

    // Off, nothing cuts, however many there are.
    app.run(simple3d_core::keymap::Command::ToggleSection);
    assert!(super::cut(&mut app, forward).is_empty());
}
