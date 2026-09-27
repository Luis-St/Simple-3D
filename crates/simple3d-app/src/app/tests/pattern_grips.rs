//! Laying a pattern out by dragging its handles.

use super::*;
use crate::gizmo::{self};
use simple3d_core::keymap::Command;
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

/// The step vector a linear pattern is laid out along.
pub(crate) fn linear_run(app: &App, pat: NodeId) -> Vec3 {
    use simple3d_core::primitive::ParamsExt;
    let params = app.scene.node(pat).params().unwrap();
    Vec3::new(params.num("step_x"), params.num("step_y"), params.num("step_z"))
}

/// The labels of the grips a pattern offers now.
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
    // Grips are placed in the node's evaluated world frame, so evaluate first.
    app.reevaluate_for_test();
    // Linear by default: spacing and count grips.
    assert_eq!(grip_labels(&app, pat), vec!["Spacing", "Copies"]);
    app.set_pattern_grip(pat, "Spacing", 84.0, free);
    // The spacing grip is at the last copy, so 84 over two gaps is a 42 mm step.
    assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(42.0)));
    app.reevaluate_for_test();
    draw_one_frame(&mut app);

    // A mirror has nothing to lay out and offers no grips.
    app.scene.get_mut(pat).unwrap().params_mut().unwrap().insert("kind".into(), ParamValue::Choice(3));
    assert!(grip_labels(&app, pat).is_empty());

    // Negative drags cannot push the step below zero.
    app.scene.get_mut(pat).unwrap().params_mut().unwrap().insert("kind".into(), ParamValue::Choice(0));
    app.set_pattern_grip(pat, "Spacing", -5.0, free);
    assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(0.0)));
}

#[test]
pub(crate) fn every_kind_that_places_copies_offers_grips_to_place_them_with() {
    // Issue 67: every kind that places copies, not only linear and grid, has viewport grips.
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
    // The count follows the pointer at the current spacing.
    use simple3d_core::primitive::ParamsExt;
    let free = gizmo::Mods { free: true, ..Default::default() };
    let mut app = headless_app();
    app.run(Command::Pattern);
    let pat = app.primary().unwrap();
    app.reevaluate_for_test();
    let step = linear_run(&app, pat).length();
    assert!(step > 1e-9);

    // The grip is one step past the last copy, so five steps out asks for five copies.
    app.set_pattern_grip(pat, "Copies", step * 5.0, free);
    assert_eq!(app.scene.node(pat).params().unwrap().int("count"), 5);
    // A part-step rounds to the nearer whole copy.
    app.set_pattern_grip(pat, "Copies", step * 2.4, free);
    assert_eq!(app.scene.node(pat).params().unwrap().int("count"), 2);
    // Dragged back past the origin it stops at one copy.
    app.set_pattern_grip(pat, "Copies", -step * 4.0, free);
    assert_eq!(app.scene.node(pat).params().unwrap().int("count"), 1);
    // The whole drag is one undo step.
    app.reevaluate_for_test();
    draw_one_frame(&mut app);
}

#[test]
pub(crate) fn a_rings_span_is_laid_out_by_carrying_its_grip_round() {
    // A ring is laid out by its radius and the arc its copies fill.
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
    // The span grip rides outside the copies, so a full turn does not overlap the radius grip.
    let grips = app.pattern_grips(pat);
    let radius = grips.iter().find(|g| g.label == "Radius").unwrap().at;
    let span = grips.iter().find(|g| g.label == "Span").unwrap().at;
    app.set_pattern_grip(pat, "Span", 360.0, free);
    assert!((radius - span).length() > 1.0, "the span grip sits on the radius grip");
}

#[test]
pub(crate) fn the_spacing_handle_lengthens_a_diagonal_run_without_straightening_it() {
    // A diagonal run stays diagonal when lengthened, and the handle stays at the last copy.
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

    // Two gaps between three copies, so 200 sets a run of 100.
    app.set_pattern_grip(pat, "Spacing", 200.0, free);
    let step = linear_run(&app, pat);
    assert!((step.length() - 100.0).abs() < 1e-6, "the run was not doubled: {step:?}");
    assert!((step - Vec3::new(60.0, 80.0, 0.0)).length() < 1e-6, "the run was straightened onto X: {step:?}");
}

#[test]
pub(crate) fn a_dragged_spacing_lands_on_the_documents_step() {
    // Regression: the spacing drag was not rounded to the move step like other drags.
    use simple3d_core::primitive::ParamValue;
    let mut app = headless_app();
    app.run(Command::Pattern);
    let pat = app.primary().unwrap();
    app.reevaluate_for_test();
    app.scene.settings.snap_step = 1.0;
    // Two gaps: the distance is rounded, then divided.
    app.set_pattern_grip(pat, "Spacing", 14.0718, gizmo::Mods::default());
    assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(7.0)));
    // Shift is the coarse step here too.
    app.set_pattern_grip(pat, "Spacing", 44.0, gizmo::Mods { coarse: true, ..Default::default() });
    assert_eq!(app.scene.node(pat).params().unwrap().get("step_x"), Some(&ParamValue::Length(20.0)));
}

#[test]
pub(crate) fn a_rings_grips_sit_on_the_copies_when_what_it_repeats_is_off_the_origin() {
    // Regression: a ring repeating a box 21 mm off the origin goes round at 51, but its grips were
    // drawn at 30 and 39, off the copies.
    use simple3d_core::primitive::ParamValue;
    let free = gizmo::Mods { free: true, ..Default::default() };
    let mut app = headless_app();
    let plate = app.primary().unwrap();
    app.scene.get_mut(plate).unwrap().position = Vec3::new(21.0, 0.0, 0.0);
    app.reevaluate_for_test();
    app.run(Command::Pattern);
    let pat = app.primary().unwrap();
    let params = app.scene.get_mut(pat).unwrap().params_mut().unwrap();
    params.insert("kind".into(), ParamValue::Choice(2));
    params.insert("circ_radius".into(), ParamValue::Length(30.0));
    app.reevaluate_for_test();

    let grips = app.pattern_grips(pat);
    let radius = grips.iter().find(|g| g.label == "Radius").expect("a ring has a radius grip");
    assert!((radius.at - Vec3::new(51.0, 0.0, 0.0)).length() < 1e-6, "the radius grip is at {:?}", radius.at);
    let span = grips.iter().find(|g| g.label == "Span").expect("a ring has a span grip");
    let (_, _, ring) = span.turn.expect("the span grip turns");
    assert!(ring > 51.0, "the span grip rides a ring of {ring} mm, inside the copies at 51");
    assert!(span.from.length() < 1e-6, "the ring still turns about the pattern's own axis, not {:?}", span.from);

    // Measured from where it stands, the grip still sets the number it shows.
    app.set_pattern_grip(pat, "Radius", 40.0, free);
    assert_eq!(app.scene.node(pat).params().unwrap().get("circ_radius"), Some(&ParamValue::Length(40.0)));
    app.reevaluate_for_test();
    let radius = app.pattern_grips(pat).into_iter().find(|g| g.label == "Radius").unwrap();
    assert!((radius.at - Vec3::new(61.0, 0.0, 0.0)).length() < 1e-6, "the radius grip is at {:?}", radius.at);
}
