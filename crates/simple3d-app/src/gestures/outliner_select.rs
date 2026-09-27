//! Selecting rows by click, Shift and Ctrl.

use super::*;
use crate::app::App;
use egui_kittest::Harness;

// -- the outliner: selecting, and the rename that is not a selection ----------

/// Three rows under the root and a harness showing them; ids in drawn order.
pub(crate) fn outliner_harness(name: &str) -> (Harness<'static, App>, Vec<simple3d_core::scene::NodeId>) {
    let mut ids = Vec::new();
    let harness = harness_stepping(name, 1.0 / 60.0, |app| {
        let root = app.scene.root();
        for (index, type_id) in ["box", "sphere", "cylinder"].iter().enumerate() {
            ids.push(app.scene.add_primitive(type_id, root, index + 1).expect("the primitive is in the registry"));
        }
        app.clear_selection();
    });
    // The shared harness's plate is the first row; these three follow.
    (harness, ids)
}

/// Let egui's double-click window pass, so the next click starts fresh.
pub(crate) fn wait_out_the_double_click_window(harness: &mut Harness<'_, App>) {
    // egui classifies clicks on its own clock, advancing a predicted frame at a time; a second is well
    // past its 0.3 s window.
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
    // Issue 59: egui detects double clicks by timing alone, so quickly clicking down a list opened a
    // rename on the second row.
    let (mut harness, ids) = outliner_harness("outliner-select-rename");

    click_row(&mut harness, ids[0]);
    assert_eq!(harness.state().selection, vec![ids[0]]);
    assert!(harness.state().rename.is_none(), "one click opened a rename");

    // Immediately afterwards, within the double-click delay, on another row.
    click_row(&mut harness, ids[1]);
    assert_eq!(harness.state().selection, vec![ids[1]], "the second row was not selected");
    assert!(harness.state().rename.is_none(), "a click on a second row opened a rename on it");

    // A real double click on one row still renames, after a pause so it is not a third click.
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
    // Issue 60: any modifier toggled one row, so ranges needed many Ctrl+clicks.
    let (mut harness, ids) = outliner_harness("outliner-modifiers");
    let rows = crate::panel_outliner::visible_rows(harness.state());
    let plate = rows[1];
    assert_eq!(rows[2..], ids[..], "this test assumes the plate and then the three shapes");

    click_row(&mut harness, plate);
    assert_eq!(harness.state().selection, vec![plate]);

    // Shift selects from the anchor to the clicked row, in tree order.
    modifiers(&mut harness, egui::Modifiers::SHIFT);
    click_row(&mut harness, ids[1]);
    assert_eq!(harness.state().selection, vec![plate, ids[0], ids[1]], "shift did not select the range");
    assert!(harness.state().rename.is_none(), "a shift-click opened a rename");

    // Shift again re-measures from the same anchor, so shortening removes rows.
    click_row(&mut harness, ids[0]);
    assert_eq!(harness.state().selection, vec![plate, ids[0]], "shift did not re-measure from the anchor");

    // Ctrl adds one row without disturbing the rest...
    modifiers(&mut harness, egui::Modifiers::COMMAND);
    click_row(&mut harness, ids[2]);
    assert_eq!(harness.state().selection, vec![plate, ids[0], ids[2]], "ctrl did not add the row");
    // ...and removes it again.
    click_row(&mut harness, ids[2]);
    assert_eq!(harness.state().selection, vec![plate, ids[0]], "ctrl did not remove the row");

    // A plain click restarts from one row.
    modifiers(&mut harness, egui::Modifiers::NONE);
    click_row(&mut harness, ids[2]);
    assert_eq!(harness.state().selection, vec![ids[2]], "a plain click did not replace the selection");
}

#[test]
pub(crate) fn a_shift_range_never_reaches_into_a_collapsed_group() {
    // The range covers visible rows only, never a collapsed group's contents.
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
