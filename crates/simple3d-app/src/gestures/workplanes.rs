//! The workplane rows' icons and the row itself (issue 125).

use super::*;
use crate::panel_workplanes::{delete_id, edit_id, row_id};
use crate::workplane_tool::WorkplaneTool;
use simple3d_core::config::Panel;

/// A harness with the Workplanes panel open and two workplanes in it, the first active.
fn with_workplanes(name: &str) -> (Harness<'static, App>, [u64; 2]) {
    let mut ids = [0; 2];
    let harness = harness_configured(name, |app| {
        app.settings.layout.collapsed.retain(|panel| *panel != Panel::Workplanes);
        for (slot, name) in ["Top", "Side"].into_iter().enumerate() {
            ids[slot] = app.scene.settings.add_workplane(WorkplaneTool::new(name.into(), 10.0).plane);
        }
        app.scene.settings.active_workplane = Some(ids[0]);
    });
    (harness, ids)
}

fn click(harness: &mut Harness<'_, App>, at: egui::Pos2) {
    press(harness, at);
    release(harness, at);
}

#[test]
pub(crate) fn the_pencil_opens_the_workplane_to_change_without_making_it_active() {
    let (mut harness, [top, side]) = with_workplanes("workplane-pencil");
    let icon = rect_of(&harness, edit_id(side)).center();
    click(&mut harness, icon);
    let app = harness.state();
    assert_eq!(app.workplane_tool.as_ref().and_then(|tool| tool.editing), Some(side), "the pencil did not open it");
    assert_eq!(app.scene.settings.active_workplane, Some(top), "the pencil also clicked the row under it");
}

#[test]
pub(crate) fn the_bin_deletes_only_its_own_workplane() {
    let (mut harness, [top, side]) = with_workplanes("workplane-bin");
    let icon = rect_of(&harness, delete_id(side)).center();
    click(&mut harness, icon);
    let settings = &harness.state().scene.settings;
    assert!(settings.workplane(side).is_none(), "the bin did not delete its workplane");
    assert!(settings.workplane(top).is_some(), "the bin deleted another workplane");
    assert_eq!(settings.active_workplane, Some(top));
}

#[test]
pub(crate) fn clicking_a_row_beside_its_icons_makes_it_active() {
    let (mut harness, [_, side]) = with_workplanes("workplane-row-click");
    let row = rect_of(&harness, row_id(side));
    click(&mut harness, egui::pos2(row.left() + 40.0, row.center().y));
    let app = harness.state();
    assert_eq!(app.scene.settings.active_workplane, Some(side), "the row click did not make it active");
    assert!(app.workplane_tool.is_none() && app.scene.settings.workplanes.len() == 2, "the row click did more");
}
