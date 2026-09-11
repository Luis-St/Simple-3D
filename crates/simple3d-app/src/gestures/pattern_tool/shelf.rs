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
    simple3d_core::pattern_library::save(&config, "Turned row", &rule, false).expect("the shelf could not be written");
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
