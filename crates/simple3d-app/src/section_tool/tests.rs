use super::grips::*;
use super::window::*;
use crate::panel_properties::component;
use simple3d_core::scene::SectionView;
use simple3d_geom::Vec3;

use super::*;

fn section(axis: usize, offset: f64) -> SectionView {
    SectionView { enabled: true, axis, offset, flipped: false }
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
