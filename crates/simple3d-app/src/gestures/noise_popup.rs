//! The scatter's own window, driven by pointer (issue 79).

use super::*;
use egui_kittest::kittest::Queryable;
use simple3d_core::keymap::Command;
use simple3d_core::pattern;
use simple3d_core::primitive::{ParamValue, ParamsExt};

/// The scatter numbers live in a window over the viewport, opened by the Noise row, instead of
/// pushing the panel's contents down.
#[test]
pub(crate) fn the_scatter_is_edited_in_a_window_over_the_viewport() {
    let mut harness = harness("noise-popup");
    harness.state_mut().run(Command::Pattern);
    for _ in 0..2 {
        harness.step();
    }
    let id = harness.state().primary().expect("the pattern is selected");
    assert!(jitter_shown(&harness).is_none(), "the jitter fields were in the panel with no window open");

    harness.get_by_label("Configure").click();
    for _ in 0..4 {
        harness.step();
    }
    assert_eq!(harness.state().noise_popup, Some(id), "the Noise row did not open the window");
    // Without noise, the window shows parts to add rather than zeroed fields (issue 79).
    assert!(jitter_shown(&harness).is_none(), "the window drew fields for a scatter nobody has added");
    let nudge = rect_of(&harness, crate::noise_popup::part_id("", crate::noise_popup::Part::Nudge));
    press(&mut harness, nudge.center());
    release(&mut harness, nudge.center());
    for _ in 0..3 {
        harness.step();
    }
    assert!(jitter_shown(&harness).is_some(), "adding a nudge did not bring its numbers up");
    let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
    assert!(params.num("noise_x") > 0.0, "adding a nudge nudged nothing");
    assert_eq!(pattern::crowding(&params, harness.state().pattern_content_size(id).unwrap()), None);

    // Kept inside the viewport it floats over.
    let window = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::new(("in-place-popup", "pattern-noise"))))
        .expect("the popup was not drawn");
    let viewport = harness.state().viewport_rect;
    assert!(
        viewport.expand(1.0).contains_rect(window),
        "the window is at {window:?}, outside the {viewport:?} it belongs to"
    );

    // The button is an action: pressing it again keeps the open window.
    harness.get_by_label("Configure").click();
    for _ in 0..2 {
        harness.step();
    }
    assert_eq!(harness.state().noise_popup, Some(id), "the Noise button flipped the window shut");
    assert!(jitter_shown(&harness).is_some(), "the numbers went away when the button was pressed again");

    // Done closes it, checked on state since a stale response survives the frame that stops drawing it.
    harness.get_by_label("Done").click();
    for _ in 0..2 {
        harness.step();
    }
    assert_eq!(harness.state().noise_popup, None, "Done left the window up");
}

/// Reset removes the whole scatter at once and one undo restores it.
#[test]
pub(crate) fn reset_takes_the_whole_scatter_off_in_one_undo_step() {
    let mut harness = harness("noise-popup-reset");
    harness.state_mut().run(Command::Pattern);
    let id = harness.state().primary().expect("the pattern is selected");
    {
        let params = harness.state_mut().scene.get_mut(id).and_then(|node| node.params_mut()).expect("a pattern");
        params.insert("noise_x".to_string(), ParamValue::Length(2.0));
        params.insert("noise_turn_z".to_string(), ParamValue::Angle(6.0));
        params.insert("noise_turn_x".to_string(), ParamValue::Angle(2.0));
        params.insert("noise_seed".to_string(), ParamValue::Count(4));
    }
    harness.state_mut().noise_popup = Some(id);
    // Two turns make a taller window that takes a few frames to settle.
    for _ in 0..8 {
        harness.step();
    }
    assert!(harness.state().noise_is_set(id), "the scatter this test is about was not set");

    harness.get_by_label("Reset").click();
    for _ in 0..2 {
        harness.step();
    }
    let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
    assert_eq!(
        pattern::Noise::of(&params),
        pattern::Noise::of(&pattern::default_params()),
        "Reset left some of the scatter behind"
    );
    assert_eq!(params.int("noise_seed"), 1, "Reset put the distances back but left the seed where it was");
    assert!(!harness.state().noise_is_set(id), "Reset left something for itself to do again");

    // One undo step for the whole scatter.
    harness.state_mut().run(Command::Undo);
    harness.step();
    let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
    assert_eq!(params.num("noise_x"), 2.0, "one undo did not bring the whole scatter back");
    assert_eq!(params.num("noise_turn_z"), 6.0);
    assert_eq!(params.num("noise_turn_x"), 2.0);
    assert_eq!(params.int("noise_seed"), 4);
}

/// Each part comes off by its own cross, and Shuffle steps the seed (issue 79).
#[test]
pub(crate) fn a_part_comes_off_by_its_own_cross_and_shuffle_steps_the_seed() {
    let mut harness = harness("noise-popup-parts");
    harness.state_mut().run(Command::Pattern);
    let id = harness.state().primary().expect("the pattern is selected");
    {
        let params = harness.state_mut().scene.get_mut(id).and_then(|node| node.params_mut()).expect("a pattern");
        params.insert("noise_x".to_string(), ParamValue::Length(2.0));
        params.insert("noise_turn_z".to_string(), ParamValue::Angle(6.0));
        params.insert("noise_turn_y".to_string(), ParamValue::Angle(4.0));
    }
    harness.state_mut().noise_popup = Some(id);
    for _ in 0..4 {
        harness.step();
    }
    let seed = |harness: &Harness<'_, App>| harness.state().scene.node(id).params().unwrap().int("noise_seed");
    let before = seed(&harness);
    let shuffle = rect_of(&harness, crate::noise_popup::shuffle_id(""));
    press(&mut harness, shuffle.center());
    release(&mut harness, shuffle.center());
    for _ in 0..2 {
        harness.step();
    }
    assert_ne!(seed(&harness), before, "Shuffle left the seed where it was");

    let cross = rect_of(&harness, crate::noise_popup::drop_part_id("", crate::noise_popup::Part::Turn(2)));
    press(&mut harness, cross.center());
    release(&mut harness, cross.center());
    for _ in 0..2 {
        harness.step();
    }
    let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
    assert_eq!(params.num("noise_turn_z"), 0.0, "the turn's cross left the turn on");
    assert_eq!(params.num("noise_turn_y"), 4.0, "the turn about Z's cross took the turn about Y with it");
    assert_eq!(params.num("noise_x"), 2.0, "the turn's cross took the nudge with it");
}

/// Whether the first jitter field is shown; queried by field since the row name includes its unit.
fn jitter_shown(harness: &Harness<'_, App>) -> Option<egui::Response> {
    harness.ctx.read_response(crate::panel_properties::grip_id("Jitter X"))
}

/// "+ Turn" adds a turn per axis (Z, X, then Y) until all three have one, and a turn's axis chips
/// move it onto a free axis.
#[test]
pub(crate) fn a_turn_is_added_about_each_axis_and_moved_between_them() {
    let mut harness = harness("noise-popup-turns");
    harness.state_mut().run(Command::Pattern);
    let id = harness.state().primary().expect("the pattern is selected");
    harness.state_mut().noise_popup = Some(id);
    for _ in 0..4 {
        harness.step();
    }
    let click = |harness: &mut Harness<'_, App>, target: egui::Id| {
        let rect = rect_of(harness, target);
        press(harness, rect.center());
        release(harness, rect.center());
        for _ in 0..3 {
            harness.step();
        }
    };
    let turns = |harness: &Harness<'_, App>| {
        let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
        pattern::NOISE_TURN_KEYS.map(|key| params.num(key) > 0.0)
    };
    click(&mut harness, crate::noise_popup::add_turn_id(""));
    assert_eq!(turns(&harness), [false, false, true], "the first turn was not about Z");
    click(&mut harness, crate::noise_popup::add_turn_id(""));
    click(&mut harness, crate::noise_popup::add_turn_id(""));
    assert_eq!(turns(&harness), [true, true, true], "three turns did not cover the three axes");
    assert!(
        harness.ctx.read_response(crate::noise_popup::add_turn_id("")).is_none(),
        "the turn chip stayed with every axis already turned"
    );

    // Remove X's turn, and Z's chips move Z's turn there.
    click(&mut harness, crate::noise_popup::drop_part_id("", crate::noise_popup::Part::Turn(0)));
    click(&mut harness, crate::noise_popup::turn_axis_id("", 2, 0));
    assert_eq!(turns(&harness), [true, true, false], "Z's turn did not move onto X");
    // Not onto Y, which has its own.
    click(&mut harness, crate::noise_popup::turn_axis_id("", 0, 1));
    assert_eq!(turns(&harness), [true, true, false], "a turn moved onto an axis already turned");
}
