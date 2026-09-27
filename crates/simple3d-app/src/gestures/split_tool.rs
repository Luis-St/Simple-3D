//! The split tool's window.

use super::*;
use crate::app::App;
use egui_kittest::Harness;
use simple3d_core::eval::{Cancel, Evaluator};
use simple3d_geom::Vec3;

/// Split the selection as the tool does, without a frame loop: open, choose, press Split, wait.
pub(crate) fn split_now(app: &mut App, tiling: simple3d_geom::tiling::Tiling) {
    app.open_split_tool();
    app.split_tool.as_mut().expect("the tool opened on the selection").plan =
        simple3d_geom::tiling::SplitPlan::of(tiling);
    app.start_split();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while app.split_job.is_some() {
        app.poll_split();
        assert!(std::time::Instant::now() < deadline, "the split never finished");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[test]
pub(crate) fn the_split_panels_button_joins_the_pieces_back_together_without_taking_the_panel_with_it() {
    use egui_kittest::kittest::Queryable;
    use simple3d_core::keymap::Command;

    // Regression: Split removed a node mid-frame while later sections still drew it ("node 6 is not in
    // the scene"), so the panel requests the command and it runs afterwards.
    let mut harness = harness_configured("split-panel", |app| {
        let root = app.scene.root();
        let group = app.scene.add_group(simple3d_core::scene::GroupOp::Union, root, 0);
        for i in 0..2 {
            let id = app.scene.add_primitive("box", group, i).unwrap();
            app.scene.get_mut(id).unwrap().position = Vec3::new(i as f64 * 60.0, 0.0, 0.0);
        }
        app.select_only(group);
        app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
        split_now(app, simple3d_geom::tiling::Tiling { size: 40.0, ..Default::default() });
    });
    let split = harness.state().primary().expect("the split is selected");
    assert!(harness.state().scene.node(split).is_split());

    let label = format!("Join back together ({})", harness.state().keymap.shortcut_text(Command::Rejoin));
    assert!(harness.query_by_label(&label).is_some(), "the split panel has no button to join the pieces back");
    harness.get_by_label(&label).click();
    harness.step();
    harness.step();

    assert!(!harness.state().scene.contains(split), "the split is still there");
    let back = harness.state().primary().expect("the object that came back is selected");
    assert!(harness.state().scene.node(back).is_group(), "what came back is not the group it was made from");
    assert_eq!(harness.state().scene.node(back).children.len(), 2, "the operands did not come back");
}

/// The tool's numbers are the usual drag-or-type field (issue 82), not plain text fields.
#[test]
pub(crate) fn the_split_tools_numbers_are_dragged_like_every_other_number() {
    use simple3d_core::keymap::Command;

    let mut harness = harness_configured("split-scrub", |app| {
        app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
    });
    harness.state_mut().run(Command::SplitIntoPieces);
    // A few frames, since the window settles over several.
    for _ in 0..6 {
        harness.step();
    }
    let size =
        |harness: &Harness<'_, App>| harness.state().split_tool.as_ref().expect("the tool is open").plan.first().size;
    assert_eq!(size(&harness), 10.0, "the tool did not open on the default cell");

    // 60 px right at 6 px per millimetre is 10 mm.
    let field = rect_of(&harness, crate::panel_properties::grip_id("split-0-size"));
    drag(&mut harness, field.center(), field.center() + egui::vec2(60.0, 0.0), 6);
    assert!((size(&harness) - 20.0).abs() < 1e-9, "the drag gave {} rather than 20 mm", size(&harness));

    // Nothing was cut: the model is untouched until Split.
    assert!(harness.state().split_job.is_none(), "dragging a number started a split");
    assert_eq!(harness.state().history.undo_len(), 0, "changing the plan left something to undo");
}

/// Several cuts, each on its own axis and cell shape (issue 82).
#[test]
pub(crate) fn a_second_cut_is_added_from_the_window_and_starts_across_the_first() {
    use egui_kittest::kittest::Queryable;
    use simple3d_core::keymap::Command;

    let mut harness = harness_configured("split-two-cuts", |app| {
        app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
    });
    harness.state_mut().run(Command::SplitIntoPieces);
    for _ in 0..6 {
        harness.step();
    }
    assert_eq!(harness.state().split_tool.as_ref().unwrap().plan.passes.len(), 1);

    harness.get_by_label("Add another cut").click();
    harness.step();
    harness.step();
    let plan = harness.state().split_tool.as_ref().expect("the tool is open").plan.clone();
    assert_eq!(plan.passes.len(), 2, "the window did not take a second cut");
    assert_ne!(plan.passes[1].axis, plan.passes[0].axis, "the second cut runs the same way as the first");

    // Both cuts show their own numbers: two Size fields.
    assert!(harness.ctx.read_response(crate::panel_properties::grip_id("split-1-size")).is_some());
    assert!(harness.query_by_label("Cut 2").is_some(), "the second cut is not named");
}

/// The split tool end to end through its window: choose, Split, and wait for the pieces (issue 82).
#[test]
pub(crate) fn the_split_tool_cuts_the_shape_into_the_cells_its_window_was_asked_for() {
    use egui_kittest::kittest::Queryable;
    use simple3d_core::keymap::Command;

    let mut harness = harness_configured("split-tool", |app| {
        app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
    });
    harness.state_mut().run(Command::SplitIntoPieces);
    // A few frames, since the window settles over several.
    for _ in 0..6 {
        harness.step();
    }
    assert!(harness.state().split_tool.is_some(), "the tool did not open");
    assert_eq!(harness.state().modal, crate::app::Modal::None, "the tool went up as a modal dialog");

    harness.get_by_label("Hexagons").click();
    harness.step();
    harness.step();
    assert_eq!(
        harness.state().split_tool.as_ref().unwrap().plan.first().kind,
        simple3d_geom::tiling::CellKind::Hexagons,
        "clicking the cell shape did not choose it"
    );

    harness.get_by_label("Split").click();
    harness.step();
    harness.step();
    assert!(harness.state().split_job.is_some(), "pressing Split did not start the cutting");
    assert!(harness.state().split_tool.is_none(), "the window stayed open over the cutting");

    // The cutting thread's result is carried back by the frame loop, driven here by hand.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while harness.state().split_job.is_some() {
        harness.state_mut().poll_split();
        harness.step();
        assert!(std::time::Instant::now() < deadline, "the split never landed");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }

    let split = harness.state().primary().expect("the split is selected");
    assert!(harness.state().scene.node(split).is_split(), "no split was made");
    assert!(harness.state().scene.node(split).children.len() > 1, "the shape came back in one piece");
    assert_eq!(
        harness.state().scene.node(split).split_plan().map(|plan| plan.first().kind),
        Some(simple3d_geom::tiling::CellKind::Hexagons),
        "the pattern the pieces were cut with was not kept"
    );
    // The new split's panel offers cutting again.
    harness.step();
    harness.step();
    assert!(harness.query_by_label("Split differently\u{2026}").is_some(), "the split's panel offers no way to re-cut");
}
