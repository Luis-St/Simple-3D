//! Cutting a shape into pieces.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::primitive::{ParamValue, ParamsExt};
use std::time::{Duration, Instant};

/// Run a split the way the application does: open the tool, choose a
/// pattern, press Split and wait for the thread to hand its pieces back.
/// The cutting is on a thread precisely so the interface does not wait for
/// it, so a test has to.
pub(crate) fn split_with(app: &mut App, tiling: simple3d_geom::tiling::Tiling) {
    use simple3d_geom::tiling::SplitPlan;
    app.run(Command::SplitIntoPieces);
    app.split_tool.as_mut().expect("the tool opened on the selection").plan = SplitPlan::of(tiling);
    app.start_split();
    let deadline = Instant::now() + Duration::from_secs(60);
    while app.split_job.is_some() {
        app.poll_split();
        assert!(Instant::now() < deadline, "the split never finished");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
pub(crate) fn splitting_a_shape_cuts_it_into_a_piece_per_cell() {
    // The half of issue 82 that cuts rather than separates: the default
    // plate is 40 x 20, so 10 mm squares through it are eight pieces, and
    // the eight of them are the plate.
    let mut app = headless_app();
    let plate = app.primary().unwrap();
    app.scene.get_mut(plate).unwrap().name = "Deck".into();
    app.reevaluate_for_test();
    let before = app.evaluated.mesh.bounds().unwrap();

    split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });

    let split = app.primary().expect("the split is selected");
    assert!(app.scene.node(split).is_split(), "the pieces did not go under a split");
    assert_eq!(app.scene.node(split).name, "Deck", "the split is not named for what it was cut from");
    assert_eq!(app.scene.node(split).children.len(), 8, "a 40 x 20 plate in 10 mm squares is eight pieces");
    for &child in &app.scene.node(split).children {
        assert!(app.scene.node(child).is_mesh());
        assert!(app.scene.node(child).mesh().unwrap().triangle_count() > 0);
    }
    // The pattern is kept on the split, which is what lets the panel say
    // what was done and the tool open again on it.
    let tiling = app.scene.node(split).split_plan().expect("the pattern was not kept").first();
    assert_eq!(tiling.kind, simple3d_geom::tiling::CellKind::Squares);
    assert_eq!(tiling.size, 10.0);
    // And the pieces are exactly where the shape was.
    app.reevaluate_for_test();
    let after = app.evaluated.mesh.bounds().unwrap();
    assert!((before.0 - after.0).length() < 1e-3, "the pieces moved: {before:?} -> {after:?}");
    assert!((before.1 - after.1).length() < 1e-3, "the pieces moved: {before:?} -> {after:?}");
}

#[test]
pub(crate) fn cutting_a_split_again_changes_the_pattern_rather_than_splitting_the_split() {
    // Opening the tool on a split offers the pattern it was cut with, and
    // cutting again replaces its pieces: the shape it was made from is
    // still the shape it was made from, so one Join back together is still
    // enough however many patterns have been tried.
    let mut app = headless_app();
    split_with(&mut app, simple3d_geom::tiling::Tiling { size: 10.0, ..Default::default() });
    let split = app.primary().unwrap();
    assert_eq!(app.scene.node(split).children.len(), 8);

    app.run(Command::SplitIntoPieces);
    let offered = app.split_tool.as_ref().expect("the tool opened on the split").plan.first();
    assert_eq!(offered.size, 10.0, "the tool did not open on the pattern the split was cut with");
    app.cancel_split_tool();

    split_with(&mut app, simple3d_geom::tiling::Tiling { size: 20.0, ..Default::default() });
    let again = app.primary().unwrap();
    assert_eq!(again, split, "cutting again made a different node");
    assert!(app.scene.node(again).is_split());
    assert_eq!(app.scene.node(again).children.len(), 2, "a 40 x 20 plate in 20 mm squares is two pieces");
    for &child in &app.scene.node(again).children {
        assert!(app.scene.node(child).is_mesh(), "a piece of the old pattern was left behind");
    }
    let original = app.scene.node(again).split_original().expect("what it was cut from");
    assert_eq!(original.type_id, "plate", "cutting again lost the shape it was made from");

    app.run(Command::Rejoin);
    let back = app.primary().unwrap();
    assert_eq!(app.scene.node(back).params().unwrap().num("width"), 40.0, "the plate did not come back whole");
}

#[test]
pub(crate) fn a_cell_bigger_than_the_shape_leaves_the_document_alone() {
    let mut app = headless_app();
    let before = app.history.undo_len();
    split_with(&mut app, simple3d_geom::tiling::Tiling { size: 500.0, ..Default::default() });
    assert_eq!(app.history.undo_len(), before, "a split that made one piece recorded an undo step");
    assert!(app.primary().is_some_and(|id| !app.scene.node(id).is_split()), "a one-piece split was made anyway");
    assert!(app.status_text().contains("one piece"), "{}", app.status_text());
}

#[test]
pub(crate) fn a_split_dropped_because_the_shape_changed_under_it_leaves_the_document_alone() {
    // The cutting runs on a thread, so the shape it was cutting can be
    // edited before the pieces land. Pieces of a shape that no longer
    // exists are not an edit anybody asked for.
    let mut app = headless_app();
    let plate = app.primary().unwrap();
    app.run(Command::SplitIntoPieces);
    app.split_tool.as_mut().unwrap().plan =
        simple3d_geom::tiling::SplitPlan::of(simple3d_geom::tiling::Tiling { size: 5.0, ..Default::default() });
    app.start_split();
    app.scene.get_mut(plate).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(90.0));
    let before = app.history.undo_len();
    let deadline = Instant::now() + Duration::from_secs(60);
    while app.split_job.is_some() {
        app.poll_split();
        assert!(Instant::now() < deadline, "the split never finished");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!app.scene.node(plate).is_split(), "pieces of the old shape were applied to the new one");
    assert_eq!(app.history.undo_len(), before, "a dropped split recorded an undo step");
    assert!(app.status_text().contains("changed while it was being cut"), "{}", app.status_text());
}

#[test]
pub(crate) fn the_pattern_a_split_was_cut_with_survives_saving_and_loading() {
    let mut app = headless_app();
    split_with(
        &mut app,
        simple3d_geom::tiling::Tiling {
            kind: simple3d_geom::tiling::CellKind::Hexagons,
            size: 12.0,
            layer: 2.0,
            ..Default::default()
        },
    );
    let text = simple3d_core::project::to_string(&app.scene);
    let scene = simple3d_core::project::from_str(&text).expect("a scene with a cut-up shape loads");
    let split = scene.depth_first().into_iter().find(|&id| scene.node(id).is_split()).expect("the split is there");
    let tiling = scene.node(split).split_plan().expect("the pattern was not written to the file").first();
    assert_eq!(tiling.kind, simple3d_geom::tiling::CellKind::Hexagons);
    assert_eq!(tiling.size, 12.0);
    assert_eq!(tiling.layer, 2.0);
}

/// Two cuts at once, end to end and through the document: the pieces are
/// what both grids leave, and the plan that made them is what the tool
/// opens on again (issue 82).
#[test]
pub(crate) fn a_shape_is_cut_by_every_cut_of_the_plan_and_the_plan_is_kept() {
    use simple3d_geom::tiling::{SplitPlan, Tiling};
    let mut app = headless_app();
    // The starting plate is 40 x 20 x 4. Squares of 20 through Z are two
    // columns; slabs of 10 through X cut each of those in two across.
    let plan = SplitPlan {
        passes: vec![
            Tiling { size: 20.0, axis: 2, ..Tiling::default() },
            Tiling { size: 10.0, axis: 0, ..Tiling::default() },
        ],
    };
    app.run(Command::SplitIntoPieces);
    app.split_tool.as_mut().expect("the tool opened on the selection").plan = plan.clone();
    app.start_split();
    let deadline = Instant::now() + Duration::from_secs(60);
    while app.split_job.is_some() {
        app.poll_split();
        assert!(Instant::now() < deadline, "the split never finished");
        std::thread::sleep(Duration::from_millis(2));
    }

    let split = app.primary().expect("the split is selected");
    assert_eq!(app.scene.node(split).children.len(), 4, "two cuts across each other are four pieces");
    assert_eq!(app.scene.node(split).split_plan(), Some(&plan), "the plan the pieces were cut by was not kept");

    // Through the file, and back out again as the plan it was.
    let text = simple3d_core::project::to_string(&app.scene);
    let scene = simple3d_core::project::from_str(&text).expect("a scene cut by two cuts loads");
    let saved = scene.depth_first().into_iter().find(|&id| scene.node(id).is_split()).expect("the split is there");
    assert_eq!(scene.node(saved).split_plan(), Some(&plan));

    // And the tool opens on both cuts rather than on the first of them.
    app.run(Command::SplitIntoPieces);
    assert_eq!(app.split_tool.as_ref().expect("the tool opened on the split").plan, plan);
}
