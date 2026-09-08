//! Laying a pattern out by dragging its handles.

use super::*;
use crate::gizmo::{self};
use simple3d_core::keymap::Command;
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

/// The step vector a linear pattern is currently laid out along.
pub(crate) fn linear_run(app: &App, pat: NodeId) -> Vec3 {
    use simple3d_core::primitive::ParamsExt;
    let params = app.scene.node(pat).params().unwrap();
    Vec3::new(params.num("step_x"), params.num("step_y"), params.num("step_z"))
}

/// The labels of the grips a pattern offers right now.
pub(crate) fn grip_labels(app: &App, pat: NodeId) -> Vec<&'static str> {
    app.pattern_grips(pat).into_iter().map(|g| g.label).collect()
}

#[test]
pub(crate) fn a_linear_patterns_spacing_is_laid_out_by_a_handle_that_follows_the_kind() {
    use simple3d_core::primitive::ParamValue;
    let free = gizmo::Mods { free: true, ..Default::default() };
    let mut app = headless_app();
    app.run(Command::Pattern);
    let pat = app.primary().unwrap();
    // A grip is placed in the node's own world frame, which comes from the
    // last evaluation, so a pattern has to have been evaluated once before it
    // offers any.
    app.reevaluate_for_test();
    // Linear by default: a grip for the spacing between copies, and one for
    // how many there are.
    assert_eq!(grip_labels(&app, pat), vec!["Spacing", "Copies"]);
    app.set_pattern_grip(pat, "Spacing", 84.0, free);
    // The spacing grip sits at the *last* copy, so the distance it is dragged
    // to is divided across the gaps: 84 over two gaps is a 42 mm step.
    assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(42.0)));
    app.reevaluate_for_test();
    draw_one_frame(&mut app);

    // A kind with nothing to lay out -- a mirror, which is a plane and two
    // copies -- offers no grips at all.
    app.scene.get_mut(pat).unwrap().params_mut().unwrap().insert("kind".into(), ParamValue::Choice(3));
    assert!(grip_labels(&app, pat).is_empty());

    // Negative drags cannot push the step below zero.
    app.scene.get_mut(pat).unwrap().params_mut().unwrap().insert("kind".into(), ParamValue::Choice(0));
    app.set_pattern_grip(pat, "Spacing", -5.0, free);
    assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(0.0)));
}

#[test]
pub(crate) fn every_kind_that_places_copies_offers_grips_to_place_them_with() {
    // Issue 67: the pattern was to have "a creation tool of its own for
    // laying one out rather than only a list of numbers in the property
    // editor". Only the linear and grid kinds ever had a viewport handle;
    // the four others -- circular and mirror among the three the issue names,
    // and the helix and spiral it asks for besides -- had none at all.
    use simple3d_core::primitive::ParamValue;
    let mut app = headless_app();
    app.run(Command::Pattern);
    let pat = app.primary().unwrap();
    app.reevaluate_for_test();
    let wanted: [(u32, &[&str]); 6] = [
        (0, &["Spacing", "Copies"]),
        (1, &["Column spacing", "Columns", "Row spacing", "Rows", "Layers"]),
        (2, &["Radius", "Span"]),
        (3, &[]),
        (4, &["Radius", "Rise", "Copies", "Turn per copy"]),
        (5, &["Start radius", "Radius per copy", "Copies", "Turn per copy"]),
    ];
    for (kind, labels) in wanted {
        app.scene.get_mut(pat).unwrap().params_mut().unwrap().insert("kind".into(), ParamValue::Choice(kind));
        assert_eq!(grip_labels(&app, pat), labels.to_vec(), "kind {kind}");
        app.reevaluate_for_test();
        draw_one_frame(&mut app);
    }
}

#[test]
pub(crate) fn dragging_the_copies_grip_lays_out_how_many_there_are() {
    // The other half of laying a pattern out by eye: the count follows the
    // pointer, at the spacing already set, instead of being typed.
    use simple3d_core::primitive::ParamsExt;
    let free = gizmo::Mods { free: true, ..Default::default() };
    let mut app = headless_app();
    app.run(Command::Pattern);
    let pat = app.primary().unwrap();
    app.reevaluate_for_test();
    let step = linear_run(&app, pat).length();
    assert!(step > 1e-9);

    // The grip is one step past the last copy, so dragging it to five steps
    // out asks for five copies.
    app.set_pattern_grip(pat, "Copies", step * 5.0, free);
    assert_eq!(app.scene.node(pat).params().unwrap().int("count"), 5);
    // A part-step lands on the nearer whole copy rather than on a fraction.
    app.set_pattern_grip(pat, "Copies", step * 2.4, free);
    assert_eq!(app.scene.node(pat).params().unwrap().int("count"), 2);
    // Dragged back past the origin it stops at one copy -- the original --
    // rather than at none or at a negative count.
    app.set_pattern_grip(pat, "Copies", -step * 4.0, free);
    assert_eq!(app.scene.node(pat).params().unwrap().int("count"), 1);
    // And the whole drag is one undo step, however many frames it took.
    app.reevaluate_for_test();
    draw_one_frame(&mut app);
}

#[test]
pub(crate) fn a_rings_span_is_laid_out_by_carrying_its_grip_round() {
    // A ring has no outward run to drag copies along, so what lays it out is
    // its radius and the arc its copies fill.
    use simple3d_core::primitive::{ParamValue, ParamsExt};
    let free = gizmo::Mods { free: true, ..Default::default() };
    let mut app = headless_app();
    app.run(Command::Pattern);
    let pat = app.primary().unwrap();
    app.scene.get_mut(pat).unwrap().params_mut().unwrap().insert("kind".into(), ParamValue::Choice(2));
    app.reevaluate_for_test();

    app.set_pattern_grip(pat, "Radius", 45.0, free);
    assert!((app.scene.node(pat).params().unwrap().num("circ_radius") - 45.0).abs() < 1e-9);
    app.set_pattern_grip(pat, "Span", 90.0, free);
    assert!((app.scene.node(pat).params().unwrap().num("circ_span") - 90.0).abs() < 1e-9);
    // The span grip rides outside the copies, so a full turn does not put it
    // on top of the radius grip, where neither could be picked out.
    let grips = app.pattern_grips(pat);
    let radius = grips.iter().find(|g| g.label == "Radius").unwrap().at;
    let span = grips.iter().find(|g| g.label == "Span").unwrap().at;
    app.set_pattern_grip(pat, "Span", 360.0, free);
    assert!((radius - span).length() > 1.0, "the span grip sits on the radius grip");
}

#[test]
pub(crate) fn the_spacing_handle_lengthens_a_diagonal_run_without_straightening_it() {
    // The handle drives the whole run, not just its X component: a pattern
    // stepping diagonally must stay diagonal when it is dragged longer, and
    // the handle must stay at the last copy rather than off along X.
    use simple3d_core::primitive::ParamValue;
    let free = gizmo::Mods { free: true, ..Default::default() };
    let mut app = headless_app();
    app.run(Command::Pattern);
    let pat = app.primary().unwrap();
    {
        let params = app.scene.get_mut(pat).unwrap().params_mut().unwrap();
        params.insert("step_x".into(), ParamValue::Length(30.0));
        params.insert("step_y".into(), ParamValue::Length(40.0));
    }
    app.reevaluate_for_test();
    assert!((linear_run(&app, pat).length() - 50.0).abs() < 1e-9, "a 3-4-5 run");

    // Two gaps between three copies, so a grip dragged to 200 sets a run of
    // 100.
    app.set_pattern_grip(pat, "Spacing", 200.0, free);
    let step = linear_run(&app, pat);
    assert!((step.length() - 100.0).abs() < 1e-6, "the run was not doubled: {step:?}");
    assert!((step - Vec3::new(60.0, 80.0, 0.0)).length() < 1e-6, "the run was straightened onto X: {step:?}");
}

#[test]
pub(crate) fn a_dragged_spacing_lands_on_the_documents_step() {
    // Every other viewport drag rounds to the move step; this one used to
    // write whatever the ray happened to hit, so laying a pattern out by eye
    // gave "7.0359 mm" under a 1 mm step.
    use simple3d_core::primitive::ParamValue;
    let mut app = headless_app();
    app.run(Command::Pattern);
    let pat = app.primary().unwrap();
    app.reevaluate_for_test();
    app.scene.settings.snap_step = 1.0;
    // Two gaps: the distance is rounded, and the step is what it divides to.
    app.set_pattern_grip(pat, "Spacing", 14.0718, gizmo::Mods::default());
    assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(7.0)));
    // Shift is the coarse step everywhere else, and here too.
    app.set_pattern_grip(pat, "Spacing", 44.0, gizmo::Mods { coarse: true, ..Default::default() });
    assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(20.0)));
}
