//! Typing into a field, and what escape and enter do to it.

use super::*;
use crate::app::App;
use egui_kittest::Harness;

/// Clicking a value field puts the whole number under the caret, so what is typed
/// next replaces it.
///
/// It opened with the caret at the end instead, so clicking a field that read 40
/// and typing 12 gave 4012. On a count with a maximum it was worse and stranger:
/// a pattern's Copies showing 7, clicked and typed "12" into, came out as 512 --
/// 712, clamped to the most copies a pattern will lay down. The field is opened
/// by a *click*, which is one gesture on the value as a whole and never a caret
/// placed anywhere in particular, so the whole value is what it opens with.
#[test]
pub(crate) fn clicking_a_value_field_puts_the_whole_number_under_the_caret() {
    use simple3d_core::primitive::ParamsExt;

    let mut harness = harness("field-select-on-open");
    let plate = harness.state().primary().expect("the starting scene has a plate selected");
    let width =
        |h: &Harness<'_, App>| h.state().scene.node(plate).params().expect("a plate has parameters").num("width");
    assert_eq!(width(&harness), 40.0, "this test types over a 40, and the field does not hold one");

    let field = rect_of(&harness, crate::panel_properties::grip_id("Width (X)"));
    press(&mut harness, field.center());
    release(&mut harness, field.center());
    // The frame after the click is the one that draws the text field and hands it
    // the keyboard, which is also where the caret is put across the value.
    harness.step();
    text(&mut harness, "12");
    key(&mut harness, egui::Key::Enter);
    harness.step();

    assert_eq!(width(&harness), 12.0, "typing 12 over a clicked field holding 40 gave {}", width(&harness));
}

#[test]
pub(crate) fn escape_abandons_a_half_typed_value_and_enter_takes_it() {
    // Escape is the way out of everything else in this application. It used to
    // be the one place it was not: egui surrenders focus on Escape, the field
    // committed on any loss of focus, and the abandoned text was written to the
    // model on the way out.
    let mut harness = harness("field-escape");
    let plate = harness.state().primary().expect("the starter shape is selected");
    let before = harness.state().scene.node(plate).position.x;

    // The field carries the name, because the field is also the grip.
    let field = rect_of(&harness, crate::panel_properties::grip_id("Position (mm):0"));
    let at = field.center();

    replace_field(&mut harness, at, "40");
    key(&mut harness, egui::Key::Escape);
    harness.step();
    assert_eq!(
        harness.state().scene.node(plate).position.x,
        before,
        "Escape committed the value that was being abandoned"
    );

    // The same typing, ended with Enter, does reach the model.
    replace_field(&mut harness, at, "40");
    key(&mut harness, egui::Key::Enter);
    harness.step();
    assert_eq!(harness.state().scene.node(plate).position.x, 40.0, "Enter did not take the typed value");
}

#[test]
pub(crate) fn a_narrow_properties_panel_stacks_its_rows_instead_of_overflowing() {
    // Issue 51. At the dock's narrowest, a label column plus three fields does
    // not fit across the panel: the fields used to shrink to slivers and the
    // last one went over the edge. The rows stack instead -- name above, one
    // axis per line -- and everything stays inside the panel.
    let mut harness = harness_configured("properties-narrow", |app| app.settings.properties_width = 200.0);
    let grips: Vec<egui::Rect> = (0..3)
        .map(|axis| rect_of(&harness, crate::panel_properties::grip_id(&format!("Position (mm):{axis}"))))
        .collect();
    // One to a line, all at the same x: three columns would put them side by
    // side at the same y.
    for pair in grips.windows(2) {
        assert!(pair[1].top() > pair[0].top(), "the axis fields are still laid out across the row: {grips:?}");
        assert!((pair[1].left() - pair[0].left()).abs() < 1.0, "the stacked fields do not line up: {grips:?}");
    }
    // And none of them is past the panel's right-hand edge.
    let right = harness.state().settings.properties_width;
    let screen = harness.ctx.screen_rect().right();
    for grip in &grips {
        assert!(grip.right() <= screen - 4.0, "a field ran off the screen: {grip:?}");
        assert!(grip.width() >= 44.0, "a field was squeezed below being readable: {grip:?} in {right}");
    }
    harness.step();
}

#[test]
pub(crate) fn a_point_row_keeps_all_three_of_its_fields_on_the_panel() {
    // Reported from the running application: dragging the dock in took the Z
    // field of the 3D cursor and of the view centre off the panel edge, while
    // the position and rotation rows above them broke onto three lines as they
    // should. Issue 57 again, in the two rows that were written after it: the
    // width for the three fields came from `available_width`, which in a wrapped
    // row is the width a *new* line would have -- the whole row, label column
    // included -- so each field was sized as though the label were not there and
    // the second and third were drawn past the panel's edge. Held against the
    // old code this test fails at the second field of a 230 px dock, which
    // reached 1404.7 with the window edge at 1400.
    //
    // Checked across the dock's whole range (`width_range(200..=620)` in
    // `dock`), because the failure was in the middle of it rather than at an end.
    for width in [200.0_f32, 230.0, 260.0, 290.0, 320.0, 420.0, 620.0] {
        // Nothing selected: the Document section, and with it the 3D cursor and
        // the view centre, is what the panel shows in place of an object's own
        // rows.
        let mut harness = harness_configured("point-row-width", |app| {
            app.settings.properties_width = width;
            app.selection.clear();
        });
        harness.step();
        let edge = harness.ctx.screen_rect().right();
        for row in ["3D cursor", "View centre"] {
            let fields: Vec<egui::Rect> = (0..3)
                .map(|axis| rect_of(&harness, crate::panel_properties::grip_id(&format!("{row}:{axis}"))))
                .collect();
            for (axis, field) in fields.iter().enumerate() {
                assert!(
                    field.right() <= edge - 4.0,
                    "at a panel {width} px wide, {row}:{axis} reaches {} with the window edge at {edge}",
                    field.right()
                );
                assert!(
                    field.left() >= 0.0 && field.width() >= 40.0,
                    "at a panel {width} px wide, {row}:{axis} is {field:?}"
                );
            }
            // And they are laid out as one thing or the other: three across on
            // one line, or one to a line down the left. A row that is half of
            // each is the shape the bug had.
            let across = fields.windows(2).all(|p| (p[1].top() - p[0].top()).abs() < 1.0);
            let stacked = fields.windows(2).all(|p| p[1].top() > p[0].top() && (p[1].left() - p[0].left()).abs() < 1.0);
            assert!(across || stacked, "at a panel {width} px wide, {row} is neither across nor stacked: {fields:?}");
        }
    }
}

#[test]
pub(crate) fn every_value_field_stays_inside_the_properties_panel_at_any_width() {
    // Issue 57. A wrapped horizontal ui reports `available_width` as the width
    // a *new* line would have, not what is left on the line being laid out --
    // so a field sized by it began after the label column and still asked for
    // the whole row. At the width this panel is usually dragged to, the
    // dimension fields ran 56 px past the window's edge and the Z column of
    // every transform row sat under it: the numbers a project is measured in
    // were off the screen.
    //
    // Checked across the widths the dock can be dragged through, because the
    // one that overflowed was not the narrowest -- the narrow ones stack, and
    // it was the wide ones that ran over.
    for width in [200.0_f32, 320.0, 420.0, 522.0, 700.0] {
        let harness = harness_configured("properties-width", |app| app.settings.properties_width = width);
        let edge = harness.ctx.screen_rect().right();
        let mut fields: Vec<(String, egui::Rect)> = Vec::new();
        for axis in 0..3 {
            for row in ["Position (mm)", "Rotation (deg)", "Scale (x)"] {
                let name = format!("{row}:{axis}");
                fields.push((name.clone(), rect_of(&harness, crate::panel_properties::grip_id(&name))));
            }
        }
        // The starter shape is a plate: its own three dimensions, and the
        // longest label of the panel among them.
        for name in ["Width (X)", "Depth (Y)", "Thickness (Z)", "Corner radius (0 = square)"] {
            fields.push((name.to_string(), rect_of(&harness, crate::panel_properties::grip_id(name))));
        }
        for (name, rect) in &fields {
            assert!(
                rect.right() <= edge - 4.0,
                "at a panel {width} px wide, {name} reaches {} with the window edge at {edge}",
                rect.right()
            );
            assert!(rect.left() >= 0.0 && rect.width() >= 40.0, "{name} at {width} px is {rect:?}");
        }
        // And every row ends at the same place. The unit is in the row's name
        // now rather than beside the field, so nothing takes a bite out of one
        // row's field that it does not take out of the others -- which is the
        // whole reason for writing it there. Measured on the last field of each
        // row, since the transform rows put three side by side.
        let last: Vec<&(String, egui::Rect)> =
            fields.iter().filter(|(name, _)| !name.contains(':') || name.ends_with(":2")).collect();
        let right = last[0].1.right();
        for (name, rect) in &last {
            assert!(
                (rect.right() - right).abs() < 1.0,
                "at a panel {width} px wide, {name} ends at {} and {} ends at {right}",
                rect.right(),
                last[0].0
            );
        }
        // The rows with one field to a line are that same width as each other,
        // whether their value carries a unit or not.
        let single: Vec<f32> =
            fields.iter().filter(|(name, _)| !name.contains(':')).map(|(_, rect)| rect.width()).collect();
        assert!(
            single.iter().all(|w| (w - single[0]).abs() < 1.0),
            "at a panel {width} px wide the dimension fields are different widths: {single:?}"
        );
    }
}
