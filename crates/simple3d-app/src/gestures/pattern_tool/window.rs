//! The tool's window itself: where it goes up, and what it opens on.

use super::*;
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

    // And the cross on its title bar puts it away without undoing anything: the
    // numbers it showed are already on the pattern.
    let pattern = harness.state().pattern_tool.expect("the tool is open on a pattern");
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

/// Opening the tool on a pattern that already has a kind starts the rule from
/// what that kind lays out, rather than throwing the layout away (issue 79).
#[test]
pub(crate) fn the_tool_opens_on_the_layout_the_pattern_already_had() {
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
    harness.step();
    let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
    assert_eq!(params.int("kind"), pattern::CUSTOM, "the tool did not switch the pattern to its own rule");
    assert_eq!(pattern::instances(&params), ring, "opening the tool laid the copies out somewhere else");

    // And any of the fixed kinds can be started from once it is open: pressing
    // Grid writes the grid the pattern is still holding as the stages that say
    // the same thing.
    let grid = {
        let mut as_grid = params.clone();
        as_grid.insert("kind".to_string(), ParamValue::Choice(1));
        pattern::instances(&as_grid)
    };
    harness.state_mut().start_rule_from(id, 1);
    harness.step();
    let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
    assert_eq!(params.int("kind"), pattern::CUSTOM);
    assert_eq!(pattern::instances(&params), grid, "starting from the grid did not lay out what the grid lays out");
}
