//! The orientation cube's zones and what they ask for.

use super::*;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn the_cube_and_the_viewport_agree_about_which_way_is_which() {
    // The previous cube rotated its own way and disagreed with the view
    // behind it by a quarter turn, which makes an orientation cube worse
    // than none. Both are driven from yaw and pitch, so this can be checked
    // directly rather than looked at.
    for (yaw, pitch) in [(-55.0, 28.0), (-90.0, 0.0), (0.0, 0.0), (140.0, -35.0), (20.0, 60.0)] {
        let mut v = view();
        v.camera.yaw = yaw;
        v.camera.pitch = pitch;
        for axis in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)] {
            let (screen, _) = v.project(v.camera.target + axis * 10.0).unwrap();
            let on_screen = screen - v.centre;
            let (on_cube, depth) = cube_project(yaw, pitch, axis, 20.0);
            assert_eq!(
                on_screen.x.abs() < 1e-3,
                on_cube.x.abs() < 1e-3,
                "{axis:?} at yaw {yaw} pitch {pitch}: one says edge-on, the other does not"
            );
            if on_screen.x.abs() > 1e-3 {
                assert_eq!(on_screen.x > 0.0, on_cube.x > 0.0, "{axis:?} points the other way across");
            }
            if on_screen.y.abs() > 1e-3 {
                assert_eq!(on_screen.y > 0.0, on_cube.y > 0.0, "{axis:?} points the other way up");
            }
            // And an axis pointing towards the eye is the one drawn in
            // front: negative depth on the cube, nearer in the viewport. An
            // axis lying edge-on has no side to be on, so it is skipped.
            if depth.abs() > 1e-6 {
                let nearer = v.to_view(v.camera.target + axis * 10.0).z < v.to_view(v.camera.target).z;
                assert_eq!(
                    nearer,
                    depth < 0.0,
                    "{axis:?} at yaw {yaw} pitch {pitch} is drawn on the wrong side of the cube"
                );
            }
        }
    }
}

#[test]
pub(crate) fn clicking_a_face_of_the_cube_asks_for_the_view_that_face_shows() {
    let reach = 20.0_f32;
    for (yaw, pitch) in [ViewPreset::Isometric.angles(), (-30.0, 15.0), (120.0, -40.0)] {
        for (index, (normal, _, label)) in CUBE_FACES.iter().enumerate() {
            let n = Vec3::new(normal[0] as f64, normal[1] as f64, normal[2] as f64);
            let (at, depth) = cube_project(yaw, pitch, n, reach);
            if depth >= 0.0 {
                // Turned away: it must not be clickable at all, or the cube
                // would answer for a face nobody can see.
                continue;
            }
            assert_eq!(cube_zone_at(yaw, pitch, at, reach), Some(*normal), "{label} at yaw {yaw} pitch {pitch}");
            let _ = index;
        }
    }
}

#[test]
pub(crate) fn a_face_turned_away_is_never_what_a_click_lands_on() {
    // Straight down the +Y axis: the back face is dead behind the front one,
    // and a click in the middle must be the front.
    let (yaw, pitch) = ViewPreset::Front.angles();
    let reach = 20.0_f32;
    let zone = cube_zone_at(yaw, pitch, egui::vec2(0.0, 0.0), reach).unwrap();
    assert_eq!(zone, [0, -1, 0], "the click landed on a face pointing away from the eye");
    assert_eq!(cube_zone_preset(zone), Some(ViewPreset::Front));
    // And a point outside the cube is not part of it at all.
    assert_eq!(cube_zone_at(yaw, pitch, egui::vec2(200.0, 0.0), reach), None);
}

#[test]
pub(crate) fn the_corners_and_edges_of_the_cube_can_be_pointed_at_too() {
    // Issue 34: a corner is how the three-quarter views are asked for, and
    // an edge is how the two beside it are.
    let reach = 20.0_f32;
    let (yaw, pitch) = ViewPreset::Isometric.angles();
    for zone in cube_zones() {
        let v = Vec3::new(zone[0] as f64, zone[1] as f64, zone[2] as f64);
        let (at, depth) = cube_project(yaw, pitch, v, reach);
        if depth >= 0.0 {
            continue;
        }
        assert_eq!(cube_zone_at(yaw, pitch, at, reach), Some(zone), "{zone:?} could not be pointed at");
    }
}

#[test]
pub(crate) fn a_corner_of_the_cube_asks_for_the_view_from_that_corner() {
    // The near corner of the isometric view is +X +Y up ... looking from
    // it means an eye 35 degrees above the ground, halfway between two
    // sides.
    let (yaw, pitch) = cube_zone_angles([1, 1, 1], 0.0);
    assert!((yaw - 45.0).abs() < 1e-6, "yaw was {yaw}");
    assert!((pitch - 35.264).abs() < 1e-3, "pitch was {pitch}");

    // An edge between two sides is halfway between them, and level.
    let (yaw, pitch) = cube_zone_angles([1, -1, 0], 0.0);
    assert!((yaw + 45.0).abs() < 1e-6, "yaw was {yaw}");
    assert_eq!(pitch, 0.0);

    // A face is still exactly the preset it always was, and the two poles
    // keep whatever yaw the camera already had -- turning the model round
    // on the way to looking straight down at it is motion for nothing.
    assert_eq!(cube_zone_angles([0, -1, 0], 17.0), ViewPreset::Front.angles());
    assert_eq!(cube_zone_angles([0, 0, 1], 17.0), (17.0, ViewPreset::Top.angles().1));
}

#[test]
pub(crate) fn every_zone_of_the_cube_is_named_after_the_sides_it_lies_between() {
    assert_eq!(cube_zone_label([0, -1, 0]), "Front");
    assert_eq!(cube_zone_label([1, -1, 0]), "right-front edge");
    assert_eq!(cube_zone_label([1, -1, 1]), "right-front-top corner");
    // Twenty-six of them, and no repeats.
    let mut labels: Vec<String> = cube_zones().iter().map(|zone| cube_zone_label(*zone)).collect();
    labels.sort();
    labels.dedup();
    assert_eq!(labels.len(), 26);
}

#[test]
pub(crate) fn every_cube_face_names_a_different_view() {
    let mut seen: Vec<&str> = Vec::new();
    for (normal, preset, label) in CUBE_FACES {
        assert!(!seen.contains(&label), "two faces are labelled {label}");
        seen.push(label);
        // The face you can see is the side the camera would be on.
        let (yaw, pitch) = preset.angles();
        let mut v = view();
        v.camera.yaw = yaw;
        v.camera.pitch = pitch;
        let eye = v.eye() - v.camera.target;
        let normal = Vec3::new(normal[0] as f64, normal[1] as f64, normal[2] as f64);
        assert!(
            eye.normalized().dot(normal) > 0.99,
            "clicking {label} would put the camera somewhere other than that face"
        );
    }
}
