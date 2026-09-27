//! What a drag carries, and what it marks on the way.

use super::*;
use crate::app::App;
use egui_kittest::Harness;
use simple3d_core::eval::{Cancel, Evaluator};
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_drag_over_the_outliner_marks_the_row_it_would_land_in_while_it_is_still_held() {
    // Issue 43: the indicator asked `hovered()`, which egui withholds during a drag, so it showed only
    // on release. Checked across several mid-drag frames.
    let mut harness = harness("outliner-drop-indicator");
    let root = harness.state().scene.root();
    let plate = harness.state().primary().expect("the starter shape is selected");
    let group = harness.state_mut().scene.add_group(simple3d_core::scene::GroupOp::Union, root, 1);
    let cube = harness.state_mut().scene.add_primitive("box", root, 2).expect("the box is in the registry");
    harness.state_mut().select_only(plate);
    harness.state_mut().toggle_selected(cube);
    harness.step();
    harness.step();

    let from = rect_of(&harness, crate::panel_outliner::row_id(cube)).center();
    press(&mut harness, from);
    // A short move first, to make the press a drag.
    move_to(&mut harness, from + egui::vec2(0.0, 4.0));
    harness.step();
    // Several frames, since a response reports the frame it was drawn in.
    for _ in 0..3 {
        let onto = rect_of(&harness, crate::panel_outliner::row_id(group)).center();
        move_to(&mut harness, onto);
        harness.step();
    }
    let target = harness.state().drop_target.expect("nothing was marked as the drop target mid-drag");
    assert_eq!(target.into, Some(group), "the group under the pointer was not marked as what would take the drop");

    let onto = rect_of(&harness, crate::panel_outliner::row_id(group)).center();
    release(&mut harness, onto);
    harness.step();
    let app = harness.state();
    assert_eq!(app.scene.node(group).children, vec![plate, cube], "the whole selection did not land in the group");
    assert!(app.drop_target.is_none(), "the drop target outlived the drag");
}

#[test]
pub(crate) fn a_pattern_grip_points_the_way_it_slides_before_it_is_grabbed() {
    // Regression: every grip showed the horizontal arrow; checked through the real viewport's cursor.
    let mut harness = harness_configured("grip-cursor", |app| {
        app.run(simple3d_core::keymap::Command::Pattern);
    });
    let pattern = harness.state().primary().expect("the tool leaves the pattern selected");
    harness.step();

    // The spacing grip's screen position for a run along a given axis.
    let cursor_over = |harness: &mut Harness<'_, App>, step: Vec3| {
        {
            let params = harness.state_mut().scene.get_mut(pattern).unwrap().params_mut().unwrap();
            for (key, value) in [("step_x", step.x), ("step_y", step.y), ("step_z", step.z)] {
                params.insert(key.into(), simple3d_core::primitive::ParamValue::Length(value));
            }
        }
        // Grips are placed in the evaluated frame, so evaluate with these numbers first.
        let app = harness.state_mut();
        app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
        // Frame the whole run, so the grip is on screen.
        app.frame_all();
        harness.step();
        let at = harness
            .state()
            .pattern_grips(pattern)
            .into_iter()
            .find(|g| g.label == "Spacing")
            .expect("a run of three copies has a spacing grip")
            .at;
        let screen = harness.state().current_view().project(at).expect("the grip is on screen").0;
        move_to(harness, screen);
        // Several frames, since a widget reports the frame it was drawn in.
        for _ in 0..3 {
            harness.step();
        }
        harness.output().platform_output.cursor_icon
    };

    // A vertical run stands up on screen too.
    assert_eq!(
        cursor_over(&mut harness, Vec3::new(0.0, 0.0, 20.0)),
        egui::CursorIcon::ResizeVertical,
        "a pattern stepping upward asked for a sideways arrow"
    );
    // A run along the ground is not vertical on screen.
    let flat = cursor_over(&mut harness, Vec3::new(20.0, 0.0, 0.0));
    assert_ne!(flat, egui::CursorIcon::ResizeVertical, "a run along the ground asked for the upright arrow");
    assert!(
        matches!(
            flat,
            egui::CursorIcon::ResizeHorizontal | egui::CursorIcon::ResizeNwSe | egui::CursorIcon::ResizeNeSw
        ),
        "a grip that slides asked for {flat:?}"
    );
}

#[test]
pub(crate) fn the_rows_a_drag_carries_stay_where_they_are_while_it_is_held() {
    // Issue 46: removing carried rows reflowed the tree under the pointer; they stay, drawn faintly.
    let mut harness = harness("outliner-drag-shadow");
    let root = harness.state().scene.root();
    let plate = harness.state().primary().expect("the starter shape is selected");
    let cube = harness.state_mut().scene.add_primitive("box", root, 1).expect("the box is in the registry");
    let last = harness.state_mut().scene.add_primitive("sphere", root, 2).expect("the sphere is in the registry");
    harness.state_mut().select_only(plate);
    harness.state_mut().toggle_selected(cube);
    harness.step();
    harness.step();

    let carried = rect_of(&harness, crate::panel_outliner::row_id(cube));
    let below = rect_of(&harness, crate::panel_outliner::row_id(last));

    press(&mut harness, carried.center());
    move_to(&mut harness, carried.center() + egui::vec2(0.0, 4.0));
    // Several frames, since the press becomes a drag a frame or two after the move.
    for _ in 0..4 {
        harness.step();
    }
    assert!(harness.state().outliner_drag.is_some(), "the drag never started");
    // Both carried rows and the row below are drawn in the same places.
    assert_eq!(rect_of(&harness, crate::panel_outliner::row_id(cube)), carried, "the dragged row left the tree");
    assert_eq!(rect_of(&harness, crate::panel_outliner::row_id(plate)).height(), carried.height());
    assert_eq!(rect_of(&harness, crate::panel_outliner::row_id(last)), below, "the tree re-flowed under the pointer");

    // Released over nothing: the drag ends and every row is normal again.
    release(&mut harness, carried.center() + egui::vec2(0.0, 4.0));
    harness.step();
    assert!(harness.state().outliner_drag.is_none(), "the drag outlived the release");
    assert_eq!(rect_of(&harness, crate::panel_outliner::row_id(last)), below);
}
