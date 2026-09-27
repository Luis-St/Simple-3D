//! Typing into a field, and what Escape and Enter do to it.

use super::*;
use crate::app::App;
use egui_kittest::Harness;

/// Clicking a value field selects the whole number, so typing replaces it. Regression: the caret
/// went to the end, so typing 12 into 7 gave 712 (clamped to 512).
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
    // The frame after the click draws the text field, focuses it and selects the value.
    harness.step();
    text(&mut harness, "12");
    key(&mut harness, egui::Key::Enter);
    harness.step();

    assert_eq!(width(&harness), 12.0, "typing 12 over a clicked field holding 40 gave {}", width(&harness));
}

#[test]
pub(crate) fn escape_abandons_a_half_typed_value_and_enter_takes_it() {
    // Regression: egui drops focus on Escape and the field committed on any focus loss.
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

    // The same typing ended with Enter reaches the model.
    replace_field(&mut harness, at, "40");
    key(&mut harness, egui::Key::Enter);
    harness.step();
    assert_eq!(harness.state().scene.node(plate).position.x, 40.0, "Enter did not take the typed value");
}

#[test]
pub(crate) fn a_narrow_properties_panel_stacks_its_rows_instead_of_overflowing() {
    // Issue 51: at the dock's narrowest the rows stack (name above, one axis per line) instead of
    // shrinking fields past the edge.
    let mut harness = harness_configured("properties-narrow", |app| app.settings.properties_width = 200.0);
    let grips: Vec<egui::Rect> = (0..3)
        .map(|axis| rect_of(&harness, crate::panel_properties::grip_id(&format!("Position (mm):{axis}"))))
        .collect();
    // One per line at the same x.
    for pair in grips.windows(2) {
        assert!(pair[1].top() > pair[0].top(), "the axis fields are still laid out across the row: {grips:?}");
        assert!((pair[1].left() - pair[0].left()).abs() < 1.0, "the stacked fields do not line up: {grips:?}");
    }
    // None past the panel's right edge.
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
    // Regression (issue 57 again): sizing by `available_width` in a wrapped row pushed the cursor's
    // and view centre's Z field past the panel edge. Checked across the dock's range, since the
    // failure was mid-range.
    for width in [200.0_f32, 230.0, 260.0, 290.0, 320.0, 420.0, 620.0] {
        // Nothing selected, so the panel shows the Document section with the cursor and view centre.
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
            // Either all across one line or one per line; a mix is the bug's shape.
            let across = fields.windows(2).all(|p| (p[1].top() - p[0].top()).abs() < 1.0);
            let stacked = fields.windows(2).all(|p| p[1].top() > p[0].top() && (p[1].left() - p[0].left()).abs() < 1.0);
            assert!(across || stacked, "at a panel {width} px wide, {row} is neither across nor stacked: {fields:?}");
        }
    }
}

#[test]
pub(crate) fn every_value_field_stays_inside_the_properties_panel_at_any_width() {
    // Issue 57: `available_width` in a wrapped row reports a new line's width, which pushed fields
    // past the window edge. Checked across widths, since the wide ones overflowed.
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
        // The starter plate's dimensions, including the panel's longest label.
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
        // Every row's last field ends at the same place, since units are in the name, not beside the field.
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
        // Single-field rows are the same width, with or without a unit.
        let single: Vec<f32> =
            fields.iter().filter(|(name, _)| !name.contains(':')).map(|(_, rect)| rect.width()).collect();
        assert!(
            single.iter().all(|w| (w - single[0]).abs() < 1.0),
            "at a panel {width} px wide the dimension fields are different widths: {single:?}"
        );
    }
}
