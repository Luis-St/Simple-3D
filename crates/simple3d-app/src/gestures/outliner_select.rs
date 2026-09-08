//! Selecting rows by click, shift and ctrl.

use super::*;
use crate::app::App;
use egui_kittest::Harness;

// -- the outliner: selecting, and the rename that is not a selection ----------

/// Three rows under the root, and the harness that shows them, for the
/// selection gestures below. Returns the ids in the order the tree draws them.
pub(crate) fn outliner_harness(name: &str) -> (Harness<'static, App>, Vec<simple3d_core::scene::NodeId>) {
    let mut ids = Vec::new();
    let harness = harness_stepping(name, 1.0 / 60.0, |app| {
        let root = app.scene.root();
        for (index, type_id) in ["box", "sphere", "cylinder"].iter().enumerate() {
            ids.push(app.scene.add_primitive(type_id, root, index + 1).expect("the primitive is in the registry"));
        }
        app.clear_selection();
    });
    // The plate the shared harness adds is the first row; these three follow it.
    (harness, ids)
}

/// Let egui's double-click window go by, so the next click starts a fresh one
/// rather than continuing the run before it. Time in the harness moves one
/// frame at a time, so waiting is stepping.
pub(crate) fn wait_out_the_double_click_window(harness: &mut Harness<'_, App>) {
    // The clock egui classifies clicks against is its own, not the raw input's
    // (the harness leaves that unset), and it moves a predicted frame at a
    // time. A second of it is well past egui's 0.3 s window.
    let now = |harness: &Harness<'_, App>| harness.ctx.input(|i| i.time);
    let start = now(harness);
    let mut frames = 0;
    while now(harness) - start < 1.0 {
        harness.step();
        frames += 1;
        assert!(frames < 10_000, "the harness's clock is not moving");
    }
}

pub(crate) fn click_row(harness: &mut Harness<'_, App>, id: simple3d_core::scene::NodeId) {
    let row = rect_of(harness, crate::panel_outliner::row_id(id));
    let at = egui::pos2(row.left() + row.width() * 0.4, row.center().y);
    press(harness, at);
    release(harness, at);
}

#[test]
pub(crate) fn a_click_on_one_row_after_a_click_on_another_selects_it_and_does_not_rename_it() {
    // Issue 59. egui decides a double click from the delay between two clicks
    // alone -- neither the position nor the widget comes into it -- so clicking
    // down a list quickly opened the rename field on whichever row was clicked
    // second, on what is a single click as far as anyone using it is concerned.
    let (mut harness, ids) = outliner_harness("outliner-select-rename");

    click_row(&mut harness, ids[0]);
    assert_eq!(harness.state().selection, vec![ids[0]]);
    assert!(harness.state().rename.is_none(), "one click opened a rename");

    // Immediately afterwards, within egui's double-click delay, on another row.
    click_row(&mut harness, ids[1]);
    assert_eq!(harness.state().selection, vec![ids[1]], "the second row was not selected");
    assert!(harness.state().rename.is_none(), "a click on a second row opened a rename on it");

    // And a real double click, both halves of it on the same row, still does.
    // After a pause, or egui reads it as the third click of a run.
    wait_out_the_double_click_window(&mut harness);
    click_row(&mut harness, ids[1]);
    assert!(harness.state().rename.is_none(), "the first click of the pair opened a rename on its own");
    click_row(&mut harness, ids[1]);
    let (renaming, buffer) = harness.state().rename.clone().expect("a double click on one row did not open a rename");
    assert_eq!(renaming, ids[1]);
    assert_eq!(buffer, harness.state().scene.node(ids[1]).name);
}

#[test]
pub(crate) fn shift_selects_the_range_and_ctrl_adds_and_removes_one_row() {
    // Issue 60: the outliner had one selection mode -- a plain click replaced
    // the selection and any modifier at all toggled a single row, so a range
    // could only be built one Ctrl+click at a time.
    let (mut harness, ids) = outliner_harness("outliner-modifiers");
    let rows = crate::panel_outliner::visible_rows(harness.state());
    let plate = rows[1];
    assert_eq!(rows[2..], ids[..], "this test assumes the plate and then the three shapes");

    click_row(&mut harness, plate);
    assert_eq!(harness.state().selection, vec![plate]);

    // Shift takes everything from the anchor to the row clicked, in the order
    // the tree shows them, and nothing else.
    modifiers(&mut harness, egui::Modifiers::SHIFT);
    click_row(&mut harness, ids[1]);
    assert_eq!(harness.state().selection, vec![plate, ids[0], ids[1]], "shift did not select the range");
    assert!(harness.state().rename.is_none(), "a shift-click opened a rename");

    // Shift again re-measures from the same anchor rather than piling ranges
    // up: shortening the range takes rows back out of it.
    click_row(&mut harness, ids[0]);
    assert_eq!(harness.state().selection, vec![plate, ids[0]], "shift did not re-measure from the anchor");

    // Ctrl adds one row without disturbing the rest...
    modifiers(&mut harness, egui::Modifiers::COMMAND);
    click_row(&mut harness, ids[2]);
    assert_eq!(harness.state().selection, vec![plate, ids[0], ids[2]], "ctrl did not add the row");
    // ...and takes it out again.
    click_row(&mut harness, ids[2]);
    assert_eq!(harness.state().selection, vec![plate, ids[0]], "ctrl did not remove the row");

    // A plain click starts again from one row, which is the third mode.
    modifiers(&mut harness, egui::Modifiers::NONE);
    click_row(&mut harness, ids[2]);
    assert_eq!(harness.state().selection, vec![ids[2]], "a plain click did not replace the selection");
}

#[test]
pub(crate) fn a_shift_range_never_reaches_into_a_collapsed_group() {
    // The range runs over the rows on screen. A group that is shut has no rows
    // to select, and picking up its contents invisibly is exactly the surprise
    // a collapsible tree exists to avoid.
    let mut inner = Vec::new();
    let mut group = None;
    let mut tail = None;
    let mut harness = harness_stepping("outliner-range-collapsed", 1.0 / 60.0, |app| {
        let root = app.scene.root();
        let g = app.scene.add_group(simple3d_core::scene::GroupOp::Union, root, 1);
        for index in 0..2 {
            inner.push(app.scene.add_primitive("box", g, index).expect("the box is in the registry"));
        }
        tail = Some(app.scene.add_primitive("sphere", root, 2).expect("the sphere is in the registry"));
        app.collapsed.insert(g);
        group = Some(g);
        app.clear_selection();
    });
    let (group, tail) = (group.unwrap(), tail.unwrap());
    let rows = crate::panel_outliner::visible_rows(harness.state());
    assert!(!rows.contains(&inner[0]), "the group is not collapsed, so this test proves nothing");

    click_row(&mut harness, group);
    modifiers(&mut harness, egui::Modifiers::SHIFT);
    click_row(&mut harness, tail);
    assert_eq!(
        harness.state().selection,
        vec![group, tail],
        "the range reached into a collapsed group, or missed a row it should have taken"
    );
}
