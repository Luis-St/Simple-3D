//! The gap between rows, and which one a drop lands in.

use super::*;

#[test]
pub(crate) fn a_long_name_widens_the_outliner_rows_instead_of_wrapping() {
    // Issue 50. A row is a fixed 22 px, so a name broken over two lines is a
    // name with its second half cut off. The rows are made as wide as the
    // longest one and the tree scrolls sideways to reach it.
    let mut harness = harness("outliner-long-name");
    let plate = harness.state().primary().expect("the starter shape is selected");
    harness.step();
    let narrow = rect_of(&harness, crate::panel_outliner::row_id(plate));

    let long = "a name far longer than any outliner panel is ever going to be wide, by some way";
    harness.state_mut().scene.get_mut(plate).unwrap().name = long.to_string();
    harness.step();
    harness.step();
    let wide = rect_of(&harness, crate::panel_outliner::row_id(plate));

    assert_eq!(wide.height(), narrow.height(), "the row grew a line instead of growing wider");
    assert!(wide.width() > narrow.width(), "the row stayed the panel's width, so the name was wrapped or cut");
}

#[test]
pub(crate) fn the_gap_under_an_open_group_drops_into_it_rather_than_beside_it() {
    // Issue 49. The gap under a group's row is the gap above its first child,
    // so what lands there belongs to the group -- it was landing in the
    // group's parent instead, a level out from where the line was drawn.
    let mut harness = harness("outliner-drop-under-group");
    let root = harness.state().scene.root();
    let plate = harness.state().primary().expect("the starter shape is selected");
    let group = harness.state_mut().scene.add_group(simple3d_core::scene::GroupOp::Union, root, 1);
    let inner = harness.state_mut().scene.add_primitive("box", group, 0).expect("the box is in the registry");
    harness.state_mut().select_only(plate);
    harness.step();
    harness.step();

    let row = rect_of(&harness, crate::panel_outliner::row_id(group));
    let from = rect_of(&harness, crate::panel_outliner::row_id(plate)).center();
    press(&mut harness, from);
    move_to(&mut harness, from + egui::vec2(0.0, 4.0));
    for _ in 0..3 {
        harness.step();
    }
    assert!(harness.state().outliner_drag.is_some(), "the drag never started");

    // The bottom edge of the group's row: the gap between it and its first child.
    let at = egui::pos2(row.center().x, row.bottom() - 1.0);
    move_to(&mut harness, at);
    harness.step();
    let target = harness.state().drop_target.expect("no drop target under the group's row");
    assert_eq!(target.parent, group, "the gap under the group named another parent");
    assert_eq!(target.index, 0, "the drop was not the group's new first child");

    release(&mut harness, at);
    harness.step();
    assert_eq!(harness.state().scene.node(group).children, vec![plate, inner], "the drop landed outside the group");
    assert_eq!(harness.state().scene.node(root).children, vec![group], "the plate stayed in the root");
}

#[test]
pub(crate) fn the_gap_between_two_rows_is_one_drop_position_all_the_way_across() {
    // Issue 48. The rows are laid out with a gap between them, and a pointer in
    // that gap is claimed by the rows either side of it -- so both drew the
    // mark, one under the upper row and one over the lower one, two orange
    // lines a spacing apart for a single landing place. Checked here both ways:
    // every step across the gap names the same drop, and the height the two
    // rows would draw their line at is the same height.
    let mut harness = harness("outliner-drop-gap");
    let root = harness.state().scene.root();
    let plate = harness.state().primary().expect("the starter shape is selected");
    let cube = harness.state_mut().scene.add_primitive("box", root, 1).expect("the box is in the registry");
    let sphere = harness.state_mut().scene.add_primitive("sphere", root, 2).expect("the sphere is in the registry");
    harness.state_mut().select_only(sphere);
    harness.step();
    harness.step();

    let upper = rect_of(&harness, crate::panel_outliner::row_id(plate));
    let lower = rect_of(&harness, crate::panel_outliner::row_id(cube));
    let gap = lower.top() - upper.bottom();
    assert!(gap > 0.0, "the rows are drawn edge to edge, so this test proves nothing");

    let from = rect_of(&harness, crate::panel_outliner::row_id(sphere)).center();
    press(&mut harness, from);
    move_to(&mut harness, from + egui::vec2(0.0, 4.0));
    for _ in 0..3 {
        harness.step();
    }
    assert!(harness.state().outliner_drag.is_some(), "the drag never started");

    // Up across the gap in small steps, the way a pointer looking for a place
    // to land actually crosses it.
    let x = upper.center().x;
    let mut y = lower.top() + 2.0;
    while y >= upper.bottom() - 2.0 {
        move_to(&mut harness, egui::pos2(x, y));
        harness.step();
        let target = harness.state().drop_target.unwrap_or_else(|| panic!("no drop target at y {y}"));
        assert_eq!(target.into, None, "the gap at y {y} was read as a drop into a node");
        assert_eq!(target.parent, root, "the gap at y {y} named another parent");
        assert_eq!(target.index, 1, "the gap at y {y} named another landing place");
        y -= gap / 4.0;
    }

    // Whichever of the two rows draws it, the line lands in the middle of the
    // gap rather than on that row's own edge, so there is only ever one of it.
    let from_above = crate::panel_outliner::gap_line_y(upper, gap, true);
    let from_below = crate::panel_outliner::gap_line_y(lower, gap, false);
    assert_eq!(from_above, from_below, "the two rows either side of the gap drew the drop line at two heights");
    assert!(from_above > upper.bottom() && from_above < lower.top(), "the line is not in the gap");

    release(&mut harness, egui::pos2(x, lower.top()));
    harness.step();
    assert_eq!(harness.state().scene.node(root).children, vec![plate, sphere, cube], "the drop landed elsewhere");
}
