use simple3d_core::config::{Layout, Panel, Side};

use super::*;

#[test]
fn a_drop_lands_above_the_header_it_is_dropped_on() {
    let headers = [20.0, 120.0, 400.0];
    assert_eq!(drop_index(&headers, 5.0), 0, "above everything means first");
    assert_eq!(drop_index(&headers, 60.0), 1);
    assert_eq!(drop_index(&headers, 300.0), 2);
    assert_eq!(drop_index(&headers, 900.0), 3, "below everything means last");
    assert_eq!(drop_index(&[], 42.0), 0, "an empty dock takes the panel at index nought");
}

#[test]
fn a_panel_moves_between_docks_and_reorders_within_one() {
    let mut layout = Layout::default();
    assert_eq!(layout.side_of(Panel::Properties), Side::Right);

    layout.move_to(Panel::Properties, Side::Left, 0);
    assert_eq!(layout.left, vec![Panel::Properties, Panel::Outliner, Panel::Primitives]);
    assert!(layout.right.is_empty());
    assert_eq!(layout.side_of(Panel::Properties), Side::Left);

    // Reordering within a dock reads the index after the panel is lifted
    // out, so this really moves it one place down.
    layout.move_to(Panel::Properties, Side::Left, 1);
    assert_eq!(layout.left, vec![Panel::Outliner, Panel::Properties, Panel::Primitives]);

    // And past the end lands at the end rather than panicking.
    layout.move_to(Panel::Outliner, Side::Right, 99);
    assert_eq!(layout.right, vec![Panel::Outliner]);
}

#[test]
fn the_filler_is_the_last_panel_that_is_not_rolled_up() {
    let mut layout = Layout::default();
    assert_eq!(layout.filler(Side::Left), Some(Panel::Primitives));
    layout.toggle_collapsed(Panel::Primitives);
    assert_eq!(layout.filler(Side::Left), Some(Panel::Outliner));
    layout.toggle_collapsed(Panel::Outliner);
    assert_eq!(layout.filler(Side::Left), None, "a dock of nothing but headers has no filler");
    layout.toggle_collapsed(Panel::Outliner);
    assert_eq!(layout.filler(Side::Left), Some(Panel::Outliner), "collapsing is a toggle");
}

#[test]
fn a_layout_that_lost_or_duplicated_a_panel_is_repaired_rather_than_left_unreachable() {
    // A settings file from another version, or one edited by hand. A panel
    // that appears nowhere would have no way back.
    let mut layout = Layout { left: vec![], right: vec![], collapsed: vec![], docks_hidden: false };
    layout.repair();
    for panel in Panel::ALL {
        assert!(layout.left.contains(&panel) || layout.right.contains(&panel), "{panel:?} is unreachable");
    }

    let mut duplicated = Layout {
        left: vec![Panel::Outliner, Panel::Outliner, Panel::Properties],
        right: vec![Panel::Outliner, Panel::Primitives],
        collapsed: vec![],
        docks_hidden: false,
    };
    duplicated.repair();
    assert_eq!(duplicated.left, vec![Panel::Outliner, Panel::Properties]);
    assert_eq!(duplicated.right, vec![Panel::Primitives]);
}

#[test]
fn hiding_the_docks_keeps_the_arrangement_exactly() {
    let mut layout = Layout::default();
    layout.move_to(Panel::Primitives, Side::Right, 0);
    layout.toggle_collapsed(Panel::Outliner);
    let before = layout.clone();
    layout.docks_hidden = true;
    layout.docks_hidden = false;
    assert_eq!(layout, before, "showing the docks again did not restore them exactly");
}
