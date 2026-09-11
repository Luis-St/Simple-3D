//! A stage's variations, added and dropped from its card by pointer (issue 79).

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::pattern::{self, Vary};

/// The chips on a stage's card add a variation card each -- the same kind twice
/// if asked -- and a card's cross takes that one off and no other.
#[test]
pub(crate) fn variations_are_added_and_dropped_from_the_stage_card() {
    let mut harness = harness("pattern-variation-cards");
    harness.state_mut().run(Command::Pattern);
    harness.step();
    let id = harness.state().primary().expect("the pattern is selected");
    harness.state_mut().open_pattern_tool();
    // Staggered planks: the rows' stage already carries its stagger.
    harness.state_mut().start_rule_from_preset(id, 0);
    harness.state_mut().pattern_tool_folded[0] = true;
    for _ in 0..4 {
        harness.step();
    }
    let rows = |harness: &Harness<'_, App>| pattern::stage(harness.state().scene.node(id).params().unwrap(), 1);
    assert_eq!(rows(&harness).varied, 1);

    let click = |harness: &mut Harness<'_, App>, target: egui::Id| {
        let rect = rect_of(harness, target);
        press(harness, rect.center());
        release(harness, rect.center());
        for _ in 0..4 {
            harness.step();
        }
    };
    click(&mut harness, crate::pattern_tool::add_variation_id(1, Vary::Spin));
    assert_eq!(rows(&harness).varied, 2, "the Spin chip added nothing");
    assert!(
        harness.ctx.read_response(crate::panel_properties::grip_id("tool:2.2 Spin")).is_some(),
        "the spin's card was not drawn"
    );
    click(&mut harness, crate::pattern_tool::add_variation_id(1, Vary::Shift));
    let kinds: Vec<Vary> = rows(&harness).variations().iter().map(|v| v.what).collect();
    assert_eq!(kinds, [Vary::Shift, Vary::Spin, Vary::Shift], "a second shift could not join the first");

    click(&mut harness, crate::pattern_tool::drop_variation_id(1, 0));
    let kinds: Vec<Vary> = rows(&harness).variations().iter().map(|v| v.what).collect();
    assert_eq!(kinds, [Vary::Spin, Vary::Shift], "the cross took off a different variation");
}
