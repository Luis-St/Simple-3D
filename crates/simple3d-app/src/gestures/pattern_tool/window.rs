//! The tool's window itself: where it goes up, and what it opens on.

use super::*;
use crate::app::App;
use egui_kittest::Harness;
use simple3d_core::keymap::Command;
use simple3d_core::pattern;
use simple3d_core::primitive::{ParamValue, ParamsExt};

/// The tool is an in-place popup over the viewport, not a dialog in front of
/// the application (issue 96).
///
/// It used to be a window most of the screen wide, half of it a second viewport
/// rendering the very same scene -- a picture of the pattern, in front of the
/// picture of the pattern, with the model it was actually being laid out on
/// blocked by a modal backdrop. Driven through the real frame, because what is
/// being checked is where the window is drawn and what it leaves reachable.
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

    // Inside the viewport it floats over, which is what "in place" means: it
    // cannot be dragged onto the other screen and left there.
    let window = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::new(("in-place-popup", "pattern-tool"))))
        .expect("the popup was not drawn");
    let viewport = harness.state().viewport_rect;
    assert!(
        viewport.expand(1.0).contains_rect(window),
        "the tool is at {window:?}, outside the {viewport:?} it belongs to"
    );

    // And Done puts it away without undoing anything: the numbers it showed are
    // already on the pattern.
    let pattern = harness.state().pattern_tool.expect("the tool is open on a pattern");
    let linear = rect_of(&harness, crate::pattern_tool::template_id(0));
    press(&mut harness, linear.center());
    release(&mut harness, linear.center());
    // Several frames: the window grows by the stages the answer brought up, and
    // a popup keeps itself inside the viewport from the height it came out at
    // last frame -- so it is still moving for a frame or two afterwards, and a
    // click aimed at where a button was before it settled lands on nothing.
    for _ in 0..4 {
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

/// The window opens by asking what the rule starts from, and shows the stages
/// only once that is answered (issue 79).
///
/// A blank form of stage numbers says nothing about what a rule is for, and
/// every rule anyone wants is one of the six fixed layouts with something added
/// to it. Until the question is answered nothing is written to the pattern
/// either, so a window opened by accident changes no numbers.
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
    // Every one of the six, and the blank sheet -- and no stage yet. Asked for
    // by id: the properties panel behind the window has a Kind row carrying the
    // same seven words.
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

    // Answered, the stages appear.
    let blank = rect_of(&harness, crate::pattern_tool::template_id(pattern::CUSTOM));
    press(&mut harness, blank.center());
    release(&mut harness, blank.center());
    harness.step();
    harness.step();
    assert_eq!(harness.state().scene.node(id).params().unwrap().int("kind"), pattern::CUSTOM);
    assert!(stage_shown(&harness).is_some(), "answering the question did not bring the stages up");
    // From nothing, which is what Custom means here: one stage, and it does not
    // move the copy it makes.
    let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
    assert_eq!(params.int("stages"), 1);
    assert_eq!(pattern::instance_count(&params).1, 1, "the blank sheet was not blank");
}

/// Starting from a fixed kind lays the rule out as exactly what that kind lays
/// out, with the numbers the pattern is already holding rather than the kind's
/// defaults (issue 79).
#[test]
pub(crate) fn starting_from_a_kind_keeps_the_layout_the_pattern_already_had() {
    let mut harness = harness("pattern-tool-template");
    harness.state_mut().run(Command::Pattern);
    let id = harness.state().primary().expect("the pattern is selected");

    // A ring of five, which is nothing a stage default says.
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

    // And the same for a grid, from the grid numbers still sitting on the node.
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

/// A pattern that is already custom has a rule, so the question is answered and
/// the window opens straight onto the stages.
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

/// Whether the first stage's own fields are on screen. Asked of a field rather
/// than of the "Stage 1" heading, which is drawn letter-spaced and uppercased
/// and so reads as nothing a label query can match.
fn stage_shown(harness: &Harness<'_, App>) -> Option<egui::Response> {
    harness.ctx.read_response(crate::panel_properties::grip_id("tool:1 Copies"))
}
