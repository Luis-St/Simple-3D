//! Dragging a field's label to change the number in it.

use super::*;
use crate::app::App;
use egui_kittest::Harness;
use simple3d_core::primitive::ParamsExt;

// -- the scrub, which is on the field itself ----------------------------------

#[test]
pub(crate) fn dragging_a_value_field_scrubs_it_in_one_undo_step() {
    let mut harness = harness("scrub");
    let plate = harness.state().primary().expect("the starting scene has a plate selected");
    let width = |harness: &Harness<'_, App>| harness.state().scene.node(plate).params().unwrap().num("width");
    assert_eq!(width(&harness), 40.0);
    let steps = harness.state().history.undo_len();

    // Sixty pixels to the right at six pixels a millimetre is ten millimetres.
    let grip = rect_of(&harness, crate::panel_properties::grip_id("Width (X)"));
    drag(&mut harness, grip.center(), grip.center() + egui::vec2(60.0, 0.0), 6);

    assert!((width(&harness) - 50.0).abs() < 1e-9, "the scrub gave {} rather than 50 mm", width(&harness));
    assert_eq!(
        harness.state().history.undo_len(),
        steps + 1,
        "a scrub across six frames left more than one thing to undo"
    );
    assert!(harness.state().scrub.id.is_none(), "the scrub was left running after the button came up");

    harness.state_mut().run(simple3d_core::keymap::Command::Undo);
    assert_eq!(width(&harness), 40.0, "one undo did not take the whole scrub back");
}

#[test]
pub(crate) fn a_scrub_that_runs_off_its_field_keeps_scrubbing_that_field_and_no_other() {
    // The pointer leaves the field almost immediately -- six pixels a
    // millimetre means any real edit crosses it -- and it passes over the other
    // two dimension fields on its way. Neither may take the gesture over.
    let mut harness = harness("scrub-away");
    let plate = harness.state().primary().unwrap();
    let params = |harness: &Harness<'_, App>| {
        let p = harness.state().scene.node(plate).params().unwrap().clone();
        (p.num("width"), p.num("depth"), p.num("thickness"))
    };
    assert_eq!(params(&harness), (40.0, 20.0, 4.0));

    let grip = rect_of(&harness, crate::panel_properties::grip_id("Width (X)"));
    let depth = rect_of(&harness, crate::panel_properties::grip_id("Depth (Y)"));
    press(&mut harness, grip.center());
    // Down onto the depth label, and 30 px across it.
    move_to(&mut harness, egui::pos2(grip.center().x, depth.center().y));
    move_to(&mut harness, egui::pos2(grip.center().x + 30.0, depth.center().y));
    release(&mut harness, egui::pos2(grip.center().x + 30.0, depth.center().y));

    let (w, d, t) = params(&harness);
    assert!((w - 45.0).abs() < 1e-9, "the field the drag began on scrubbed to {w} rather than 45 mm");
    assert_eq!((d, t), (20.0, 4.0), "a field the pointer merely crossed was scrubbed too");
}

#[test]
pub(crate) fn every_scrubbable_value_is_dragged_from_its_own_field() {
    // Issue 27: the drag used to be on whatever sat beside the field -- the
    // label for a dimension, the four-pixel axis chip for a position -- so the
    // same gesture had a different grip depending on the row. Every one of them
    // is now the field, and this drags all three kinds to prove it.
    let mut harness = harness("scrub-everywhere");
    let plate = harness.state().primary().expect("the starting scene has a plate selected");
    let node = |harness: &Harness<'_, App>| harness.state().scene.node(plate).clone();
    let before = node(&harness);

    for (name, moved) in [("Position (mm):0", 10.0), ("Rotation (deg):2", 150.0), ("Scale (x):1", 0.5)] {
        let field = rect_of(&harness, crate::panel_properties::grip_id(name));
        drag(&mut harness, field.center(), field.center() + egui::vec2(60.0, 0.0), 6);
        let after = node(&harness);
        let change = match name {
            "Position (mm):0" => after.position.x - before.position.x,
            "Rotation (deg):2" => after.rotation.z - before.rotation.z,
            _ => after.scale.y - before.scale.y,
        };
        assert!((change - moved).abs() < 1e-9, "dragging {name} moved it by {change} rather than {moved}");
    }
}

#[test]
pub(crate) fn a_setting_scrubs_on_one_axis_with_the_same_modifiers_as_a_dimension() {
    // The document's own numbers -- the step, the grid, the segment default, the
    // 3D cursor, the ends of the measure span -- were egui's `DragValue`, which
    // is a different control: it counted vertical movement as a second axis to
    // edit on and it had no coarse modifier. They come through the same field as
    // every dimension now, and the step is the one that is on screen with a
    // shape selected, so it is the one dragged here.
    let mut harness = harness("scrub-setting");
    let step = |harness: &Harness<'_, App>| harness.state().scene.settings.snap_step;
    assert_eq!(step(&harness), 1.0);
    let history = harness.state().history.undo_len();

    // Straight down, which used to change the value and must now do nothing at
    // all -- including leaving a step behind that undoes nothing.
    let field = rect_of(&harness, crate::panel_properties::grip_id("Step"));
    drag(&mut harness, field.center(), field.center() + egui::vec2(0.0, 60.0), 6);
    assert_eq!(step(&harness), 1.0, "a drag straight down the field changed the step");
    assert_eq!(harness.state().history.undo_len(), history, "a drag that changed nothing left an undo step");

    // Sixty pixels to the right at six pixels a step is ten millimetres, and the
    // whole drag is one thing to undo.
    let field = rect_of(&harness, crate::panel_properties::grip_id("Step"));
    drag(&mut harness, field.center(), field.center() + egui::vec2(60.0, 0.0), 6);
    assert!((step(&harness) - 11.0).abs() < 1e-9, "the scrub gave {} rather than 11 mm", step(&harness));
    assert_eq!(harness.state().history.undo_len(), history + 1, "a scrub across six frames left more than one step");
    harness.state_mut().run(simple3d_core::keymap::Command::Undo);
    assert_eq!(step(&harness), 1.0, "one undo did not take the whole scrub back");

    // And the coarse modifier, which egui's control did not have: the same sixty
    // pixels are worth ten times as much with Ctrl held.
    let field = rect_of(&harness, crate::panel_properties::grip_id("Step"));
    modifiers(&mut harness, egui::Modifiers::COMMAND);
    drag(&mut harness, field.center(), field.center() + egui::vec2(60.0, 0.0), 6);
    modifiers(&mut harness, egui::Modifiers::NONE);
    assert!((step(&harness) - 101.0).abs() < 1e-9, "a coarse scrub gave {} rather than 101 mm", step(&harness));
}

#[test]
pub(crate) fn the_turn_step_is_a_field_beside_the_move_step_and_costs_no_undo() {
    // Issue 98: fifteen degrees was fixed because there was nowhere to change
    // it, so the row is what the fix is -- under the rotation fields it
    // governs, next to the step that governs a move. It is a user setting
    // rather than part of the document, so unlike the step beside it a scrub
    // through it leaves the history alone.
    let mut harness = harness("scrub-turn-step");
    let turn = |harness: &Harness<'_, App>| harness.state().settings.rotate_snap_deg;
    assert_eq!(turn(&harness), 15.0, "the default this test is about has changed");
    let history = harness.state().history.undo_len();

    // Sixty pixels to the right at six pixels a step is ten degrees.
    let field = rect_of(&harness, crate::panel_properties::grip_id("Turn"));
    drag(&mut harness, field.center(), field.center() + egui::vec2(60.0, 0.0), 6);
    assert!((turn(&harness) - 25.0).abs() < 1e-9, "the scrub gave {} rather than 25 degrees", turn(&harness));
    assert_eq!(harness.state().history.undo_len(), history, "changing a user setting left an undo step");

    // And it cannot be dragged down to nothing: a step of zero is a division by
    // zero in every snap that reads it.
    let field = rect_of(&harness, crate::panel_properties::grip_id("Turn"));
    drag(&mut harness, field.center(), field.center() + egui::vec2(-600.0, 0.0), 6);
    assert!(turn(&harness) > 0.0, "the turn step was dragged to {}", turn(&harness));
}

/// A count follows the pointer at the rate every other field does.
///
/// A count is stored whole, and each frame of a scrub read the stored value back
/// out of the model before adding that frame's movement to it -- so the fraction
/// of a step a single frame is worth was rounded away sixty times a second
/// rather than added up. A hand moving a pixel a frame rounded to nothing and the
/// field never moved at all; a hand moving four rounded *up* every frame and the
/// field ran away from the pointer, which is what made the pattern's numbers feel
/// unlike every other number in the application. Sixty pixels of drag put twelve
/// copies on a pattern that should have gained ten, and a slow drag across the
/// same sixty put on none.
///
/// Both halves are the same claim, so the test is one comparison: one step is one
/// copy and it is also one millimetre, so a count dragged a given distance has to
/// change by what a length field dragged the same distance changes by. The
/// pattern offers both on adjacent rows.
#[test]
pub(crate) fn a_count_field_follows_the_pointer_at_the_rate_every_other_field_does() {
    for (pixels, frames) in [(12.0_f32, 12usize), (60.0, 6), (60.0, 40)] {
        let mut harness = harness("count-scrub-rate");
        harness.state_mut().run(simple3d_core::keymap::Command::Pattern);
        harness.step();
        harness.step();
        let pattern = harness.state().primary().expect("the command leaves the new pattern selected");
        let value = |h: &Harness<'_, App>, key: &str| {
            use simple3d_core::primitive::ParamsExt;
            h.state().scene.node(pattern).params().expect("a pattern carries parameters").num(key)
        };
        let (copies, step) = (value(&harness, "count"), value(&harness, "step_x"));

        let count_field = rect_of(&harness, crate::panel_properties::grip_id("Copies"));
        drag(&mut harness, count_field.center(), count_field.center() + egui::vec2(pixels, 0.0), frames);
        let length_field = rect_of(&harness, crate::panel_properties::grip_id("Step X"));
        drag(&mut harness, length_field.center(), length_field.center() + egui::vec2(pixels, 0.0), frames);

        let gained = value(&harness, "count") - copies;
        let moved = value(&harness, "step_x") - step;
        assert!(gained > 0.0, "{pixels} px over {frames} frames did not move the count at all");
        // Within half a step, which is all that can be left between them once
        // the fraction is carried rather than dropped: the count is following the
        // same accumulated pointer movement, and rounding it to a whole one is
        // the only difference. Not rounded and compared exactly, because a drag
        // that lands on a half is on the rounding boundary and the two sides of
        // it disagree by a part in a hundred million million.
        const HALF: f64 = 0.5 + 1e-6;
        assert!(
            (gained - moved).abs() <= HALF,
            "{pixels} px over {frames} frames gained {gained} copies \
             where the millimetre field on the next row moved {moved}"
        );
    }
}
