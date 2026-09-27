//! The tool's window: where it opens, and what it opens on.

use super::*;
use crate::app::App;
use egui_kittest::Harness;
use simple3d_core::keymap::Command;
use simple3d_core::pattern;
use simple3d_core::primitive::{ParamValue, ParamsExt};

/// The tool is an in-place popup over the viewport, not a modal dialog with a second viewport
/// (issue 96). Driven through a real frame to check placement and reachability.
#[test]
pub(crate) fn the_pattern_tool_goes_up_over_the_viewport_rather_than_in_front_of_it() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("pattern-tool-popup");
    harness.state_mut().run(Command::Pattern);
    harness.state_mut().open_pattern_tool();
    for _ in 0..4 {
        harness.step();
    }
    assert!(harness.state().pattern_tool.is_some(), "the tool did not open");
    assert_eq!(harness.state().modal, crate::app::Modal::None, "the tool went up as a modal dialog");

    // Kept inside the viewport it floats over.
    let window = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::new(("in-place-popup", "pattern-tool"))))
        .expect("the popup was not drawn");
    let viewport = harness.state().viewport_rect;
    assert!(
        viewport.expand(1.0).contains_rect(window),
        "the tool is at {window:?}, outside the {viewport:?} it belongs to"
    );

    // Done closes it without undoing anything: the numbers are already on the pattern.
    let pattern = harness.state().pattern_tool.expect("the tool is open on a pattern");
    let linear = rect_of(&harness, crate::pattern_tool::template_id(0));
    press(&mut harness, linear.center());
    release(&mut harness, linear.center());
    // Several frames: the popup keeps moving for a frame or two as it grows and clamps itself,
    // and a click aimed at an unsettled button lands on nothing.
    for _ in 0..8 {
        harness.step();
    }
    harness.get_by_label("Done").click();
    harness.step();
    harness.step();
    assert!(harness.state().pattern_tool.is_none(), "Done left the window up");
    assert_eq!(
        harness.state().scene.node(pattern).params().unwrap().int("kind"),
        pattern::CUSTOM,
        "closing the window took the rule with it"
    );
}

/// The window first asks what the rule starts from and shows stages only once answered (issue 79);
/// nothing is written before that, so an accidental open changes nothing.
#[test]
pub(crate) fn the_window_asks_what_to_start_from_before_it_shows_any_stages() {
    use egui_kittest::kittest::Queryable;

    let mut harness = harness("pattern-tool-start-from");
    harness.state_mut().run(Command::Pattern);
    let id = harness.state().primary().expect("the pattern is selected");
    harness.state_mut().open_pattern_tool();
    for _ in 0..4 {
        harness.step();
    }
    // All six plus the blank sheet, and no stage yet. By id, since the properties panel has the same words.
    for kind in 0..=pattern::CUSTOM {
        assert!(
            harness.ctx.read_response(crate::pattern_tool::template_id(kind)).is_some(),
            "{} is not offered to start from",
            pattern::KINDS[kind as usize]
        );
    }
    assert!(stage_shown(&harness).is_none(), "the stages were shown before the question was answered");
    assert!(harness.query_by_label("Add a stage").is_none(), "a stage could be added before there was a rule");
    assert_eq!(
        harness.state().scene.node(id).params().unwrap().int("kind"),
        0,
        "asking the question already changed the pattern"
    );

    let blank = rect_of(&harness, crate::pattern_tool::template_id(pattern::CUSTOM));
    press(&mut harness, blank.center());
    release(&mut harness, blank.center());
    harness.step();
    harness.step();
    assert_eq!(harness.state().scene.node(id).params().unwrap().int("kind"), pattern::CUSTOM);
    assert!(stage_shown(&harness).is_some(), "answering the question did not bring the stages up");
    // The question disappears once answered, as its buttons would discard the edits.
    for kind in 0..=pattern::CUSTOM {
        assert!(
            harness.ctx.read_response(crate::pattern_tool::template_id(kind)).is_none(),
            "{} was still offered after the question had been answered",
            pattern::KINDS[kind as usize]
        );
    }
    // Custom starts from nothing: one stage that does not move its copy.
    let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
    assert_eq!(params.int("stages"), 1);
    assert_eq!(pattern::instance_count(&params).1, 1, "the blank sheet was not blank");
}

/// A pattern made by the custom-pattern menu item is custom and blank from the start: one copy,
/// every number zero. Regression: it first read "Linear", then was a linear layout named Custom.
#[test]
pub(crate) fn a_pattern_made_for_the_tool_starts_custom_and_blank() {
    let mut harness = harness("pattern-tool-born-custom");
    // A shape is selected, so the tool makes a pattern, as Add > Custom pattern does.
    harness.state_mut().open_pattern_tool();
    for _ in 0..4 {
        harness.step();
    }
    let id = harness.state().pattern_tool.expect("the tool did not open on a pattern");
    let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
    assert_eq!(params.int("kind"), pattern::CUSTOM, "the pattern the custom tool made calls itself something else");

    // One stage, one copy, exactly where the original stands.
    assert_eq!(params.int("stages"), 1, "a rule built by hand started with more than one stage on it");
    let copies = pattern::instances(&params);
    assert_eq!(copies.len(), 1, "a blank rule laid down {} copies", copies.len());
    assert_eq!(
        copies[0].xform,
        simple3d_core::xform::Xform::IDENTITY,
        "the one copy was moved by a rule saying nothing"
    );
    for stage in 0..pattern::MAX_STAGES {
        assert_eq!(
            pattern::stage(&params, stage),
            pattern::Stage::run(1, simple3d_geom::Vec3::ZERO),
            "stage {} of a blank rule carries numbers nobody typed",
            stage + 1
        );
    }

    // The rule is still unstarted, so the window still asks what to start from.
    assert!(!harness.state().pattern_tool_started, "the question was answered by the pattern being made");
    for kind in 0..=pattern::CUSTOM {
        assert!(
            harness.ctx.read_response(crate::pattern_tool::template_id(kind)).is_some(),
            "{} is not offered to start from",
            pattern::KINDS[kind as usize]
        );
    }
    assert!(stage_shown(&harness).is_none(), "the stages were shown before the question was answered");

    // Starting from a fixed kind uses the numbers the fixed kinds still hold.
    harness.state_mut().start_rule_from(id, 0);
    harness.step();
    let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
    assert_eq!(pattern::instance_count(&params).1, 3, "starting from linear did not lay out what linear lays out");
}

/// Starting from a fixed kind lays out exactly that kind, with the pattern's current numbers
/// rather than defaults (issue 79).
#[test]
pub(crate) fn starting_from_a_kind_keeps_the_layout_the_pattern_already_had() {
    let mut harness = harness("pattern-tool-template");
    harness.state_mut().run(Command::Pattern);
    let id = harness.state().primary().expect("the pattern is selected");

    // A ring of five, which no stage default gives.
    {
        let params = harness.state_mut().scene.get_mut(id).and_then(|n| n.params_mut()).expect("a pattern");
        params.insert("kind".to_string(), ParamValue::Choice(2));
        params.insert("circ_count".to_string(), ParamValue::Count(5));
        params.insert("circ_radius".to_string(), ParamValue::Length(30.0));
        params.insert("circ_span".to_string(), ParamValue::Angle(360.0));
    }
    let ring = pattern::instances(harness.state().scene.node(id).params().unwrap());
    harness.step();

    harness.state_mut().open_pattern_tool();
    harness.state_mut().start_rule_from(id, 2);
    harness.step();
    let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
    assert_eq!(params.int("kind"), pattern::CUSTOM, "starting the rule did not switch the pattern to it");
    assert_eq!(pattern::instances(&params), ring, "starting from circular laid the copies out somewhere else");

    // The same for a grid, from the grid numbers on the node.
    let grid = {
        let mut as_grid = params.clone();
        as_grid.insert("kind".to_string(), ParamValue::Choice(1));
        pattern::instances(&as_grid)
    };
    harness.state_mut().start_rule_from(id, 1);
    harness.step();
    let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
    assert_eq!(pattern::instances(&params), grid, "starting from the grid did not lay out what the grid lays out");
}

/// An already custom pattern opens straight onto its stages.
#[test]
pub(crate) fn a_pattern_that_already_has_a_rule_opens_on_its_stages() {
    let mut harness = harness("pattern-tool-existing-rule");
    harness.state_mut().run(Command::Pattern);
    let id = harness.state().primary().expect("the pattern is selected");
    harness
        .state_mut()
        .scene
        .get_mut(id)
        .and_then(|node| node.params_mut())
        .expect("a pattern has parameters")
        .insert("kind".to_string(), ParamValue::Choice(pattern::CUSTOM));

    harness.state_mut().open_pattern_tool();
    for _ in 0..4 {
        harness.step();
    }
    assert!(stage_shown(&harness).is_some(), "a rule that exists was hidden behind the question");
}

/// The first stage's field, if shown. Queried by field since the "Stage 1" heading is
/// letter-spaced and uppercased, which label queries do not match.
fn stage_shown(harness: &Harness<'_, App>) -> Option<egui::Response> {
    harness.ctx.read_response(crate::panel_properties::grip_id("tool:1 Copies"))
}
