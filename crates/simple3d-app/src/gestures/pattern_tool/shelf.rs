//! Picking a saved rule, and framing the preview.

use super::*;
use simple3d_core::primitive::ParamsExt;

#[test]
pub(crate) fn a_custom_pattern_picks_a_saved_kind_from_the_panel() {
    use egui_kittest::kittest::Queryable;

    // Asked for from the running application: a kind saved from the tool is
    // meant to be used again, and reaching one should not mean opening the tool
    // -- picking it is a single click on a name. The shelf the tool keeps is
    // offered on the pattern itself, beside the button that edits it.
    let mut harness = harness("pattern-saved-kind-panel");
    harness.state_mut().add_pattern();
    harness.step();
    let id = harness.state().primary().expect("the new pattern is selected");

    // A rule on the shelf: two stages, twelve copies -- the same shape as the
    // one the tool's own test saves, and nothing a fixed kind lays out.
    let mut rule = simple3d_core::pattern::default_params();
    rule.insert("stages".to_string(), simple3d_core::primitive::ParamValue::Count(2));
    rule.insert("stage1_count".to_string(), simple3d_core::primitive::ParamValue::Count(3));
    rule.insert("stage2_count".to_string(), simple3d_core::primitive::ParamValue::Count(4));
    rule.insert("stage2_turn".to_string(), simple3d_core::primitive::ParamValue::Angle(90.0));
    let config = harness.state().config_dir().to_path_buf();
    simple3d_core::pattern_library::save(&config, "Turned row", &rule).expect("the shelf could not be written");
    harness.state_mut().refresh_pattern_kinds();

    // The shelf is offered on a custom pattern only: a linear one has a kind of
    // its own and its numbers to say it.
    harness.step();
    assert!(
        harness.ctx.read_response(crate::panel_properties::saved_kind_id()).is_none(),
        "a linear pattern offers the custom shelf"
    );

    harness
        .state_mut()
        .scene
        .get_mut(id)
        .and_then(|node| node.params_mut())
        .expect("a pattern has parameters")
        .insert("kind".to_string(), simple3d_core::primitive::ParamValue::Choice(simple3d_core::pattern::CUSTOM));
    harness.step();

    // Nothing is picked yet, so the box says so rather than naming a kind.
    let box_rect = rect_of(&harness, crate::panel_properties::saved_kind_id());
    press(&mut harness, box_rect.center());
    release(&mut harness, box_rect.center());
    harness.step();
    harness.step();
    harness.get_by_label("Turned row").click();
    harness.step();
    harness.step();

    let applied = harness.state().scene.node(id).params().cloned().expect("a pattern has parameters");
    assert_eq!(applied.int("kind"), simple3d_core::pattern::CUSTOM);
    assert_eq!(
        simple3d_core::pattern::instance_count(&applied).1,
        12,
        "the saved rule was not put on the pattern: {applied:?}"
    );
    // And the pattern is named after what was picked, which is what the box
    // reads back -- the same thing the tool's own shelf shows.
    assert_eq!(harness.state().scene.node(id).name, "Turned row", "the pattern was not named after the kind");
    assert!(
        harness.ctx.read_response(crate::panel_properties::saved_kind_id()).is_some(),
        "the box went once a kind was on the pattern"
    );
}

/// The Frame button puts the preview back the way it opened -- the angle and
/// the zoom with the rest, not just where the camera is pointed.
///
/// Asked for after the button was found to re-centre an orbited picture and
/// leave it orbited, which is half a reset and reads as a button that half
/// works. The automatic reframing, when a rule starts laying its copies out
/// somewhere else, still leaves the angle alone: that one happens under the
/// user's hands, and turning the picture mid-orbit is not what they asked for.
///
/// Driven through the button itself rather than through
/// `App::reset_pattern_preview`, because what is being checked is the wiring:
/// the dialog is `Embedded` against a headless context, so the tool is drawn
/// where the harness can click it.
#[test]
pub(crate) fn the_pattern_tools_frame_button_puts_the_angle_and_the_zoom_back() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("pattern-preview-frame");
    harness.state_mut().open_pattern_tool();
    harness.step();
    harness.step();
    assert_eq!(harness.state().modal, crate::app::Modal::PatternKind, "the tool did not open");

    // The picture opens at the viewport's angle. Turn it away and pull it out,
    // the way looking round a pattern does.
    let opened = harness.state().pattern_preview_camera;
    assert_eq!(opened.yaw, harness.state().scene.camera.yaw, "the preview did not open at the viewport's angle");
    {
        let camera = &mut harness.state_mut().pattern_preview_camera;
        camera.yaw += 73.0;
        camera.pitch += 21.0;
        camera.distance *= 4.0;
    }
    harness.step();

    harness.get_by_label("Frame").click();
    harness.step();
    harness.step();

    let now = harness.state().pattern_preview_camera;
    let viewport = harness.state().scene.camera;
    assert_eq!(now.yaw, viewport.yaw, "Frame left the picture turned away from the viewport's angle");
    assert_eq!(now.pitch, viewport.pitch, "Frame left the picture pitched away from the viewport's angle");
    assert!(
        (now.distance - opened.distance).abs() < 1e-6,
        "Frame left the picture at {} rather than the {} it opened at",
        now.distance,
        opened.distance
    );
    assert_eq!(harness.state().scene.camera, viewport, "framing the preview moved the viewport behind it");
}
