//! The scatter's own window, driven by pointer (issue 79).

use super::*;
use egui_kittest::kittest::Queryable;
use simple3d_core::keymap::Command;
use simple3d_core::pattern;
use simple3d_core::primitive::{ParamValue, ParamsExt};

/// The six numbers live in a window over the viewport rather than unrolled into
/// the properties panel, and the panel's Noise row is what puts it up.
///
/// Asked for from the running application. In the panel the fields appeared by
/// pushing everything under them down the page -- to edit something whose whole
/// effect is in the viewport the panel was now covering less of.
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
    assert!(jitter_shown(&harness).is_some(), "the window went up without the numbers in it");

    // Inside the viewport it floats over, which is what "in place" means: it
    // cannot be dragged onto the other screen and left there.
    let window = harness
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::new(("in-place-popup", "pattern-noise"))))
        .expect("the popup was not drawn");
    let viewport = harness.state().viewport_rect;
    assert!(
        viewport.expand(1.0).contains_rect(window),
        "the window is at {window:?}, outside the {viewport:?} it belongs to"
    );

    // The row's button is an action, not a toggle: pressing it again asks for
    // the same window, which is already up, and nothing goes away under the
    // pointer that just asked for it.
    harness.get_by_label("Configure").click();
    for _ in 0..2 {
        harness.step();
    }
    assert_eq!(harness.state().noise_popup, Some(id), "the Noise button flipped the window shut");
    assert!(jitter_shown(&harness).is_some(), "the numbers went away when the button was pressed again");

    // The window closes the way every in-place popup closes: by its own foot.
    // Asked of the state rather than of the fields, which is what the tool's own
    // window test asks: a response the context has already handed out survives
    // the frame that stops drawing it.
    harness.get_by_label("Done").click();
    for _ in 0..2 {
        harness.step();
    }
    assert_eq!(harness.state().noise_popup, None, "Done left the window up");
}

/// Reset takes the whole scatter off at once, and one undo puts all of it back.
///
/// Asked for from the running application: four numbers typed back to zero by
/// hand was the only way to stop a pattern being scattered, and three of them
/// left at a tenth of a millimetre is a pattern that still is.
#[test]
pub(crate) fn reset_takes_the_whole_scatter_off_in_one_undo_step() {
    let mut harness = harness("noise-popup-reset");
    harness.state_mut().run(Command::Pattern);
    let id = harness.state().primary().expect("the pattern is selected");
    {
        let params = harness.state_mut().scene.get_mut(id).and_then(|node| node.params_mut()).expect("a pattern");
        params.insert("noise_x".to_string(), ParamValue::Length(2.0));
        params.insert("noise_turn".to_string(), ParamValue::Angle(6.0));
        params.insert("noise_seed".to_string(), ParamValue::Count(4));
    }
    harness.state_mut().noise_popup = Some(id);
    for _ in 0..4 {
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

    // One step, not four: a scatter is taken off as a whole, so it comes back as
    // a whole.
    harness.state_mut().run(Command::Undo);
    harness.step();
    let params = harness.state().scene.node(id).params().cloned().expect("a pattern");
    assert_eq!(params.num("noise_x"), 2.0, "one undo did not bring the whole scatter back");
    assert_eq!(params.num("noise_turn"), 6.0);
    assert_eq!(params.int("noise_seed"), 4);
}

/// Whether the first jitter's own field is on screen. Asked of the field rather
/// than of the row's name, which is drawn with its unit in brackets and so reads
/// as nothing a label query can match.
fn jitter_shown(harness: &Harness<'_, App>) -> Option<egui::Response> {
    harness.ctx.read_response(crate::panel_properties::grip_id("Jitter X"))
}
