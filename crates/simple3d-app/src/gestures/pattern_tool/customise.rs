//! Reaching the tool from a fixed kind, and what pointing at a stage shows
//! (issue 79).

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::pattern;
use simple3d_core::primitive::ParamsExt;

/// A pattern of a fixed kind offers the tool too -- as "Customise", which opens
/// it on the question of what to start from and writes nothing until that is
/// answered, so the Kind row stays the one place a kind is chosen.
///
/// Before, the only way to start a rule from a fixed kind a second time was to
/// set the Kind row back to it and go through Add > Custom pattern with the
/// pattern selected.
#[test]
pub(crate) fn a_fixed_kind_opens_the_tool_from_customise_without_changing_kind() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("pattern-customise");
    harness.state_mut().run(Command::Pattern);
    harness.step();
    let id = harness.state().primary().expect("the pattern is selected");
    assert!(harness.query_by_label("Edit kind").is_none(), "a linear pattern offers the custom kind's editor");

    harness.get_by_label("Customise").click();
    for _ in 0..4 {
        harness.step();
    }
    assert_eq!(harness.state().pattern_tool, Some(id), "Customise did not open the tool on the pattern");
    assert!(!harness.state().pattern_tool_started, "the tool skipped the question of what to start from");
    assert_eq!(
        harness.state().scene.node(id).params().unwrap().int("kind"),
        0,
        "opening the tool changed the pattern's kind"
    );
    for preset in 0..pattern::PRESETS.len() {
        assert!(
            harness.ctx.read_response(crate::pattern_tool::preset_id(preset)).is_some(),
            "{} is not offered to start from",
            pattern::PRESETS[preset]
        );
    }
}

/// Pointing at a stage in the tool marks, in the viewport, the copies the rule
/// has made by the end of that stage -- which is what "each stage repeats what
/// the ones above it made" looks like.
#[test]
pub(crate) fn pointing_at_a_stage_marks_the_copies_made_by_the_end_of_it() {
    let mut harness = harness("pattern-stage-hover");
    harness.state_mut().run(Command::Pattern);
    harness.step();
    let id = harness.state().primary().expect("the pattern is selected");
    harness.state_mut().open_pattern_tool();
    harness.state_mut().start_rule_from_preset(id, 2);
    for _ in 0..4 {
        harness.step();
    }
    assert_eq!(harness.state().pattern_tool_hover, None, "a stage was marked with the pointer nowhere near it");

    let second = rect_of(&harness, crate::pattern_tool::fold_stage_id(1));
    move_to(&mut harness, second.center());
    harness.step();
    assert_eq!(harness.state().pattern_tool_hover, Some(1), "pointing at the second stage did not mark it");
    let first = rect_of(&harness, crate::pattern_tool::fold_stage_id(0));
    move_to(&mut harness, first.center());
    harness.step();
    assert_eq!(harness.state().pattern_tool_hover, Some(0));

    // And its heading folds it: the fields go and the stage stays.
    press(&mut harness, first.center());
    release(&mut harness, first.center());
    harness.step();
    assert!(harness.state().pattern_tool_folded[0], "clicking the heading did not fold the stage");
    assert!(
        harness.ctx.read_response(crate::panel_properties::grip_id("tool:1 Copies")).is_none(),
        "a folded stage still drew its fields"
    );
}

/// The saved kinds are offered in the start question through the same dropdown
/// the properties panel uses, and picking one from it answers the question.
///
/// Asked for from the running application: the question listed them as chips,
/// while the panel's Rule row offered the very same shelf as a dropdown.
#[test]
pub(crate) fn the_start_question_offers_saved_kinds_in_the_panels_dropdown() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("pattern-start-saved");
    let mut rule = pattern::default_params();
    rule.insert("stages".to_string(), simple3d_core::primitive::ParamValue::Count(2));
    rule.insert("stage2_count".to_string(), simple3d_core::primitive::ParamValue::Count(4));
    let config = harness.state().config_dir().to_path_buf();
    simple3d_core::pattern_library::save(&config, "Four rows", &rule, false).expect("the shelf could not be written");
    harness.state_mut().refresh_pattern_kinds();

    harness.state_mut().run(Command::Pattern);
    harness.step();
    let id = harness.state().primary().expect("the pattern is selected");
    harness.state_mut().open_pattern_tool();
    for _ in 0..4 {
        harness.step();
    }
    assert!(!harness.state().pattern_tool_started);
    let shelf = rect_of(&harness, crate::pattern_tool::saved_start_id());
    press(&mut harness, shelf.center());
    release(&mut harness, shelf.center());
    harness.step();
    harness.step();
    harness.get_by_label("Four rows").click();
    for _ in 0..3 {
        harness.step();
    }
    assert!(harness.state().pattern_tool_started, "picking a saved kind did not answer the question");
    let params = harness.state().scene.node(id).params().cloned().unwrap();
    assert_eq!(params.int("kind"), pattern::CUSTOM);
    assert_eq!(params.int("stages"), 2, "the saved kind was not put on the pattern");
}
