//! The tool in place of a pattern's stages.

use super::*;
use crate::app::App;
use egui_kittest::Harness;
use simple3d_core::primitive::ParamsExt;

#[test]
pub(crate) fn a_custom_pattern_offers_the_tool_in_place_of_its_stages() {
    use egui_kittest::kittest::Queryable;

    // A custom kind's thirty-odd numbered stage fields are not listed in the Pattern section; the
    // tool edits them beside what they make.
    let mut harness = harness("pattern-custom-rows");
    harness.state_mut().add_pattern();
    harness.step();
    let id = harness.state().primary().expect("the new pattern is selected");

    // A fixed kind still shows its numbers.
    assert!(
        harness.ctx.read_response(crate::panel_properties::grip_id("Copies")).is_some(),
        "a linear pattern lost the numbers that place it"
    );
    // And offers no way into the tool; the old button made the pattern custom as a side effect.
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

    // All four stages are gone.
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
    // Left: the way into the tool, the kind, and the summary line.
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

    // Kind and axis options flow across their row and are still buttons; units in the row names keep
    // every field the same width. Checked against the real panel.
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

    // Circular has unitless Copies, degree Span and millimetre Radius, which must line up.
    harness.get_by_label("Circular").click();
    harness.step();
    harness.step();
    // Across the kinds, since a spiral's "Radius per copy (mm)" is the longest label.
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

    // The axis options are buttons on their own row too.
    for (index, name) in ["X", "Y", "Z"].iter().enumerate() {
        harness.get_by_label(name).click();
        harness.step();
        harness.step();
        let axis = harness.state().scene.node(pattern).params().unwrap().int("circ_axis");
        assert_eq!(axis, index as u32, "clicking axis {name} did not choose it");
    }
}
