//! The measure tool's window and its clicks.

use super::*;
use simple3d_geom::Vec3;

// -- issues 78 and 86: the measure tool's own window ---------------------------

#[test]
pub(crate) fn the_measure_window_is_there_only_while_the_tool_is_out() {
    use egui_kittest::kittest::Queryable;

    // Issue 78: the span is numbers that can be typed -- and nowhere at all once
    // the tool is put away. Issue 86: they are in the tool's own in-place popup
    // over the viewport rather than in a section of the property panel, which
    // describes the selection and has nothing to do with a measurement. The
    // window is found by the button only it has; a title the popup paints is not
    // in the accessibility tree to ask about.
    let mut harness = harness("measure-window");
    assert!(harness.query_by_label("Done").is_none(), "the window was there with the tool put away");

    harness.state_mut().run(simple3d_core::keymap::Command::MeasureTool);
    harness.step();
    harness.step();
    assert!(harness.query_by_label("Done").is_some(), "the tool is out and its window is not");

    // The window is wired to the tool, not just drawn over it.
    harness.get_by_label("Done").click();
    harness.step();
    harness.step();
    assert!(!harness.state().measure.active, "the window's own button did not put the tool away");
    assert!(harness.query_by_label("Done").is_none(), "the window outlived the tool");
}

#[test]
pub(crate) fn a_right_click_in_the_viewport_takes_the_last_measure_point_back() {
    let mut harness = harness("measure-unplace");
    harness.state_mut().run(simple3d_core::keymap::Command::MeasureTool);
    harness.step();

    // Place both ends by clicking the viewport, the way the tool is used.
    let centre = harness.state().viewport_rect.center();
    let (a, b) = (centre - egui::vec2(60.0, 30.0), centre + egui::vec2(60.0, 30.0));
    press(&mut harness, a);
    release(&mut harness, a);
    press(&mut harness, b);
    release(&mut harness, b);
    assert_eq!(harness.state().measure.points.len(), 2, "the two clicks did not place two ends");

    // A right-click takes the last one off, and the next takes the other.
    button(&mut harness, b, egui::PointerButton::Secondary, true);
    button(&mut harness, b, egui::PointerButton::Secondary, false);
    harness.step();
    assert_eq!(harness.state().measure.points.len(), 1, "the right-click did not take the end back");
    button(&mut harness, b, egui::PointerButton::Secondary, true);
    button(&mut harness, b, egui::PointerButton::Secondary, false);
    harness.step();
    assert!(harness.state().measure.points.is_empty(), "the start is still placed");
    assert!(harness.state().measure.active, "the tool was put away by a right-click");
}

#[test]
pub(crate) fn the_measure_window_shows_the_span_and_takes_it_back() {
    use egui_kittest::kittest::Queryable;

    // The ends are editable fields, so they are spin buttons: three for the
    // start and three for the end, and they read what the tool holds.
    let mut harness = harness("measure-fields");
    harness.state_mut().run(simple3d_core::keymap::Command::MeasureTool);
    harness.state_mut().measure.set_point(0, Vec3::new(1.0, 2.0, 3.0));
    harness.state_mut().measure.set_point(1, Vec3::new(11.0, 2.0, 3.0));
    harness.step();
    harness.step();

    let shown: Vec<String> = harness
        .get_all_by_role(egui::accesskit::Role::SpinButton)
        .filter_map(|n| n.value().map(|v| v.to_string()))
        .collect();
    for expected in ["1", "2", "3", "11"] {
        assert!(shown.iter().any(|v| v == expected), "no field reads {expected}: {shown:?}");
    }

    // Clearing from the window takes the span away without putting the tool away.
    harness.get_by_label("Clear the span").click();
    harness.step();
    assert!(harness.state().measure.points.is_empty(), "the window's Clear left the span in place");
    assert!(harness.state().measure.active, "clearing the span also put the tool away");
}
