//! Sliding the section plane by each of its grips.

use super::*;
use crate::app::App;
use egui_kittest::Harness;
use simple3d_geom::Vec3;

// -- the section plane's grips ------------------------------------------------

/// Where the section plane's grips are drawn right now, on screen.
pub(crate) fn section_grips(harness: &Harness<'_, App>) -> Vec<egui::Pos2> {
    let view = harness.state().current_view();
    let corners =
        crate::section_tool::frame(&harness.state().scene.settings.section, harness.state().evaluated.mesh.bounds());
    crate::section_tool::grips(&corners).iter().filter_map(|&at| view.project(at).map(|(p, _)| p)).collect()
}

#[test]
pub(crate) fn every_edge_grip_slides_the_section_plane() {
    // Four grips on the edges rather than one, because which of them the model
    // is standing in front of depends entirely on where it has been orbited to
    // (issue 72). Each one is dragged in turn: a grip that is drawn and does
    // nothing is worse than no grip.
    let mut harness = harness("section-edge-grips");
    let plate = harness.state().primary().expect("the fixture puts a plate in");
    harness.state_mut().clear_selection();
    harness.state_mut().run(simple3d_core::keymap::Command::ToggleSection);
    harness.step();

    let axis = harness.state().scene.settings.section.axis();
    let mut along = Vec3::ZERO;
    crate::panel_properties::set_component(&mut along, axis, 1.0);

    for index in 0..4 {
        let at = section_grips(&harness)[index];
        let view = harness.state().current_view();
        let travel = crate::panel_viewport::screen_direction(&view, harness.state().scene.camera.target, along);
        assert!(travel.length() > 1e-3, "the plane's axis is edge-on to the camera; the drag would say nothing");
        let before = harness.state().scene.settings.section.offset;

        drag(&mut harness, at, at + travel.normalized() * 60.0, 4);

        let after = harness.state().scene.settings.section.offset;
        assert!((after - before).abs() > 1e-9, "the grip on edge {index} slid the plane nowhere: still at {after}");
        assert_eq!(harness.state().scene.node(plate).position, Vec3::ZERO, "grip {index} moved the model instead");
        assert!(harness.state().selection.is_empty(), "grip {index} selected what was behind the plane");
    }
}

#[test]
pub(crate) fn the_grip_in_the_middle_slides_the_plane_too() {
    // The fifth grip, which is the one nearest to hand when the plane is being
    // looked at face-on and its edges are out at the sides of the picture. It
    // is offered the pointer before the manipulator is, and keeps it: a handle
    // that lands here is one pointing at the camera, which cannot be dragged in
    // this view whoever gets the press.
    let mut harness = harness("section-middle-grip");
    harness.state_mut().clear_selection();
    harness.state_mut().run(simple3d_core::keymap::Command::ToggleSection);
    harness.step();

    let at = section_grips(&harness)[4];
    let view = harness.state().current_view();
    let axis = harness.state().scene.settings.section.axis();
    let mut along = Vec3::ZERO;
    crate::panel_properties::set_component(&mut along, axis, 1.0);
    let travel = crate::panel_viewport::screen_direction(&view, harness.state().scene.camera.target, along);
    let before = harness.state().scene.settings.section.offset;

    drag(&mut harness, at, at + travel.normalized() * 60.0, 4);

    let after = harness.state().scene.settings.section.offset;
    assert!((after - before).abs() > 1e-9, "the grip in the middle of the plane slid it nowhere: still at {after}");
}
