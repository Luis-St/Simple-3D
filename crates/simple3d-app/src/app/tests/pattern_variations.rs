//! Building a stage's variations and adding stages as what they do, in the
//! tool (issue 79).

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::pattern::{self, StageMode, Vary};

fn with_rule() -> (App, simple3d_core::scene::NodeId) {
    let mut app = headless_app();
    app.open_pattern_tool();
    let pattern = app.pattern_tool.expect("the tool opened on a pattern");
    app.start_rule_from(pattern, 0);
    app.reevaluate_for_test();
    (app, pattern)
}

fn params(app: &App, id: simple3d_core::scene::NodeId) -> simple3d_core::primitive::Params {
    app.scene.node(id).params().cloned().expect("a pattern has parameters")
}

/// Asked for from the running application: a stage could only have one "Vary"
/// section. It now takes a list -- two shifts and a spin here -- each one added
/// doing something, any one dropped, and each change one step of undo.
#[test]
pub(crate) fn a_stage_takes_several_variations_and_drops_any_of_them() {
    let (mut app, pattern) = with_rule();
    app.add_stage_doing(pattern::StageMode::Move);
    app.add_variation(1, Vary::Shift);
    app.add_variation(1, Vary::Shift);
    app.add_variation(1, Vary::Spin);
    let stage = pattern::stage(&params(&app, pattern), 1);
    assert_eq!(stage.varied, 3, "the stage did not take three variations");
    assert!(stage.variations().iter().all(|v| v.acts(StageMode::Move)), "a variation was added doing nothing");

    let before = params(&app, pattern);
    app.drop_variation(1, 0);
    assert_eq!(pattern::stage(&params(&app, pattern), 1).variations(), &stage.variations()[1..]);
    app.run(Command::Undo);
    assert_eq!(params(&app, pattern), before, "undo did not bring the variation back");

    // Four is the most; a fifth asks for nothing, not even an undo step.
    app.add_variation(1, Vary::Gap);
    let full = params(&app, pattern);
    app.add_variation(1, Vary::Size);
    assert_eq!(params(&app, pattern), full, "a fifth variation changed the rule");
    app.run(Command::Undo);
    assert_eq!(pattern::variation_count(&params(&app, pattern), 1), 3, "the refused fifth took an undo step");
}

/// "Add a stage" offers the three things a stage can do, and the stage comes
/// in doing it -- sized to the shape rather than as one copy in place.
#[test]
pub(crate) fn a_stage_is_added_as_what_it_does() {
    let (mut app, pattern) = with_rule();
    app.add_stage_doing(StageMode::Turn);
    app.add_stage_doing(StageMode::Mirror);
    app.add_stage_doing(StageMode::Move);
    let rule = params(&app, pattern);
    assert_eq!(pattern::stage_count(&rule), 4);
    assert_eq!(pattern::stage(&rule, 1).mode, StageMode::Turn);
    assert!(pattern::stage(&rule, 1).radius > 0.0, "the ring that was added has no radius");
    assert_eq!(pattern::stage(&rule, 2).mode, StageMode::Mirror);
    let run = pattern::stage(&rule, 3);
    assert_eq!(run.mode, StageMode::Move);
    assert!(run.step.length() > 0.0 && run.count >= 2, "the run that was added goes nowhere");

    app.add_stage_doing(StageMode::Move);
    assert_eq!(pattern::stage_count(&params(&app, pattern)), 4, "a fifth stage was added");
}
