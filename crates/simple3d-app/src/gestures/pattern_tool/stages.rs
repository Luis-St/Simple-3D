//! The tool in place of a pattern's stages.

use super::*;
use crate::app::App;
use egui_kittest::Harness;
use simple3d_core::primitive::ParamsExt;

#[test]
pub(crate) fn a_custom_pattern_offers_the_tool_in_place_of_its_stages() {
    use egui_kittest::kittest::Queryable;

    // Asked for from the running application. A custom kind's rule is thirty-odd
    // numbered fields -- "1 Copies", "3 Radius per copy" -- and the Pattern
    // section listed every one of them under the kind row, where they say
    // nothing about the rule they make and are only a wall to scroll past on the
    // way to the button that opens the tool. The tool is where they are edited,
    // and it draws each stage beside what it lays down.
    let mut harness = harness("pattern-custom-rows");
    harness.state_mut().add_pattern();
    harness.step();
    let id = harness.state().primary().expect("the new pattern is selected");

    // A fixed kind still shows its numbers: this is about the custom one only.
    assert!(
        harness.ctx.read_response(crate::panel_properties::grip_id("Copies")).is_some(),
        "a linear pattern lost the numbers that place it"
    );
    // And it is offered no way into the tool. The button used to sit under every
    // kind reading "Custom kind...", which made becoming custom something that
    // happened on the way to opening a tool rather than a kind the user picked:
    // the Kind row is where that is said.
    assert!(harness.query_by_label("Edit kind").is_none(), "a linear pattern offers the custom kind's tool");

    let custom = simple3d_core::pattern::CUSTOM;
    harness
        .state_mut()
        .scene
        .get_mut(id)
        .and_then(|node| node.params_mut())
        .expect("a pattern has parameters")
        .insert("kind".to_string(), simple3d_core::primitive::ParamValue::Choice(custom));
    harness.step();

    // The stages are gone, all four of them.
    for stage in 0..simple3d_core::pattern::MAX_STAGES {
        for key in simple3d_core::pattern::stage_keys(stage) {
            let spec = simple3d_core::pattern::PARAMS.iter().find(|p| p.key == key).expect("a stage key with no spec");
            assert!(
                harness.ctx.read_response(crate::panel_properties::grip_id(spec.label)).is_none(),
                "the Pattern section still draws {}",
                spec.label
            );
        }
    }
    assert!(
        harness.ctx.read_response(crate::panel_properties::grip_id("Stages")).is_none(),
        "the Pattern section still draws the stage count"
    );
    // What is left is the way in to the tool, the kind itself, and the line
    // saying what the pattern makes.
    assert!(harness.query_by_label("Edit kind").is_some(), "the way into the tool went with the stages");
    assert!(harness.query_by_label("Custom").is_some(), "the kind row went with the stages");
    assert!(
        harness.query_by_label_contains("Put shapes under this pattern").is_some(),
        "the line saying what the pattern makes went with the stages"
    );
}

#[test]
pub(crate) fn a_patterns_kind_is_clicked_from_a_row_and_its_fields_are_all_one_width() {
    use egui_kittest::kittest::Queryable;

    // Two layout changes to the pattern editor, both checked against the real
    // panel because both are about where things are drawn.
    //
    // The kind and axis options flow across their row instead of taking a line
    // each -- so the first thing to prove is that they are still *buttons* after
    // being lifted out of the vertical layout they used to sit in.
    //
    // And the unit is in the row's name, in brackets, rather than after the
    // field. That is what makes every field one width: a count carries no unit
    // and a step does, and the two used to end at different places down the same
    // column.
    let mut harness = harness_configured("pattern-panel", |app| {
        app.run(simple3d_core::keymap::Command::Pattern);
    });
    let pattern = harness.state().primary().expect("the tool leaves the pattern selected");
    let kind = |harness: &Harness<'_, App>| harness.state().scene.node(pattern).params().unwrap().int("kind");
    assert_eq!(kind(&harness), 0, "a fresh pattern is linear");

    // Every kind is on the row, and clicking one chooses it.
    for (index, name) in ["Linear", "Grid", "Circular", "Mirror", "Helix", "Spiral"].iter().enumerate() {
        assert!(harness.query_by_label(name).is_some(), "{name} is not on the kind row");
        harness.get_by_label(name).click();
        harness.step();
        harness.step();
        assert_eq!(kind(&harness), index as u32, "clicking {name} did not choose it");
    }

    // A circular pattern is the case the width rule is about: Copies has no
    // unit, Span is in degrees and Radius in millimetres, and all three fields
    // have to start and end together.
    harness.get_by_label("Circular").click();
    harness.step();
    harness.step();
    //
    // Checked across the kinds, not just this one: the longest names a pattern
    // has are a spiral's, and "Radius per copy (mm)" is the one a bracketed unit
    // could have pushed out of the label column and into the field beside it.
    let mut fields: Vec<(&str, egui::Rect)> = Vec::new();
    for (kind, names) in [
        ("Circular", &["Copies", "Span", "Radius"][..]),
        ("Spiral", &["Copies", "Angle per copy", "Start radius", "Radius per copy", "Rise per copy"][..]),
    ] {
        harness.get_by_label(kind).click();
        harness.step();
        harness.step();
        for name in names {
            fields.push((name, rect_of(&harness, crate::panel_properties::grip_id(name))));
        }
    }
    for pair in fields.windows(2) {
        let ((a_name, a), (b_name, b)) = (pair[0], pair[1]);
        assert!(
            (a.width() - b.width()).abs() < 0.5 && (a.right() - b.right()).abs() < 0.5,
            "{a_name} and {b_name} are different sizes: {a:?} and {b:?}"
        );
    }

    harness.get_by_label("Circular").click();
    harness.step();
    harness.step();

    // And the axis options are buttons on their own row, the same as the kinds.
    for (index, name) in ["X", "Y", "Z"].iter().enumerate() {
        harness.get_by_label(name).click();
        harness.step();
        harness.step();
        let axis = harness.state().scene.node(pattern).params().unwrap().int("circ_axis");
        assert_eq!(axis, index as u32, "clicking axis {name} did not choose it");
    }
}
