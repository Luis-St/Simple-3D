//! Editing a rule's stages in the tool, starting it over, and keeping its
//! scatter on the shelf (issue 79).

use super::*;
use simple3d_core::pattern;
use simple3d_core::primitive::{ParamValue, ParamsExt};
use simple3d_geom::Vec3;

/// An app with the tool open on a pattern of the starting shape, its rule
/// started from a linear run.
fn with_rule() -> (App, simple3d_core::scene::NodeId) {
    let mut app = headless_app();
    app.open_pattern_tool();
    let pattern = app.pattern_tool.expect("the tool opened on a pattern");
    app.start_rule_from(pattern, 0);
    app.reevaluate_for_test();
    (app, pattern)
}

fn params(app: &App, id: simple3d_core::scene::NodeId) -> simple3d_core::primitive::Params {
    app.scene.node(id).params().cloned().expect("a pattern has parameters")
}

/// A stage that is added starts as the next thing the rule is missing -- a run
/// along an axis nothing above it runs along, spaced clear of the shape -- and
/// not as whatever its slot last held.
///
/// Asked for from the analysis of the tool: after a blank rule, "Add a stage"
/// put one copy in place, which looked like the button doing nothing.
#[test]
pub(crate) fn a_stage_that_is_added_runs_along_a_free_axis_clear_of_the_shape() {
    let (mut app, pattern) = with_rule();
    let size = app.pattern_content_size(pattern).expect("the pattern holds the starting shape");
    app.add_stage_doing(pattern::StageMode::Move);
    let second = pattern::stage(&params(&app, pattern), 1);
    assert_eq!(second.mode, pattern::StageMode::Move);
    assert!(
        (second.step - Vec3::new(0.0, size.y * 1.5, 0.0)).length() < 1e-9,
        "the new stage ran {:?}, not across Y clear of a shape {size:?}",
        second.step
    );
    let first = pattern::stage(&params(&app, pattern), 0).copies();
    assert_eq!(pattern::instance_count(&params(&app, pattern)).1, first * 2, "the new stage made no copies");

    // After a blank rule too: the one stage goes nowhere, so the new one takes X.
    app.start_rule_from(pattern, pattern::CUSTOM);
    app.add_stage_doing(pattern::StageMode::Move);
    let fresh = pattern::stage(&params(&app, pattern), 1);
    assert!(fresh.step.x > 0.0 && fresh.count >= 2, "after a blank rule the new stage was {fresh:?}");
}

/// Any stage can be dropped and the ones below move up; any can move; and
/// undo takes each back. A stage's fold goes with the stage, not its slot.
#[test]
pub(crate) fn a_stage_can_be_dropped_or_moved_from_anywhere_and_undone() {
    let (mut app, pattern) = with_rule();
    app.add_stage_doing(pattern::StageMode::Move);
    app.add_stage_doing(pattern::StageMode::Move);
    let before = params(&app, pattern);
    let third = pattern::stage(&before, 2);
    app.pattern_tool_folded = [false, false, true, false];

    app.drop_stage(1);
    let after = params(&app, pattern);
    assert_eq!(pattern::stage_count(&after), 2);
    assert_eq!(pattern::stage(&after, 1), third, "the stage below the one dropped did not move up");
    assert!(app.pattern_tool_folded[1], "the fold stayed with the slot instead of the stage");
    app.run(simple3d_core::keymap::Command::Undo);
    assert_eq!(params(&app, pattern), before, "undo did not bring the dropped stage back");

    let (one, two) = (pattern::stage(&before, 0), pattern::stage(&before, 1));
    app.move_stage(1, true);
    assert_eq!(pattern::stage(&params(&app, pattern), 0), two);
    assert_eq!(pattern::stage(&params(&app, pattern), 1), one);
    // The first stage cannot move up, nor the last down.
    let moved = params(&app, pattern);
    app.move_stage(0, true);
    app.move_stage(2, false);
    assert_eq!(params(&app, pattern), moved, "a stage moved off the end of the stack");
}

/// "Start over" asks the question again and writes nothing until it is
/// answered; the rule it was asked over is one click away.
#[test]
pub(crate) fn starting_over_asks_again_and_changes_nothing_until_answered() {
    let (mut app, pattern) = with_rule();
    app.add_stage_doing(pattern::StageMode::Move);
    let rule = params(&app, pattern);

    app.start_rule_over();
    assert!(!app.pattern_tool_started, "the question did not come back");
    assert!(app.pattern_tool_resumable);
    assert_eq!(params(&app, pattern), rule, "asking the question again changed the rule");
    app.resume_rule();
    assert!(app.pattern_tool_started && !app.pattern_tool_resumable);

    // Answered with a ready-made layout, the rule is replaced by it, sized to
    // what the pattern repeats.
    app.start_rule_over();
    app.start_rule_from_preset(pattern, 0);
    assert!(app.pattern_tool_started && !app.pattern_tool_resumable);
    let planks = params(&app, pattern);
    let size = app.pattern_content_size(pattern).unwrap();
    assert_eq!(pattern::stage_count(&planks), 2);
    assert!(pattern::stage(&planks, 0).step.x > size.x, "the planks were not laid out clear of the shape");
    assert!(pattern::Noise::of(&planks).wanted(), "the planks came without their scatter");
    // And the stage that staggers them does it with a variation of its own,
    // which is the card the builder draws the stagger on.
    assert!(pattern::stage(&planks, 1).varies(), "the planks' rows are not staggered");
}

/// Saved with its noise, a kind brings the noise back wherever it is used;
/// saved without, it leaves the other pattern's own alone.
#[test]
pub(crate) fn a_saved_kind_keeps_the_noise_only_when_it_was_saved_with_it() {
    let dir = temp_config_dir("pattern-kind-noise");
    let mut app = app_in(dir.clone());
    let root = app.scene.root();
    let shape = app.scene.add_primitive("box", root, 0).unwrap();
    app.select_only(shape);
    app.open_pattern_tool();
    let pattern = app.pattern_tool.unwrap();
    app.start_rule_from(pattern, 0);
    app.scene.get_mut(pattern).and_then(|n| n.params_mut()).unwrap().insert("noise_x".into(), ParamValue::Length(0.7));

    app.pattern_tool_keep_noise = true;
    app.pattern_tool_name = "Scattered".into();
    app.save_current_kind();
    app.pattern_tool_keep_noise = false;
    app.pattern_tool_name = "Bare".into();
    app.save_current_kind();

    let mut other = app_in(dir);
    let root = other.scene.root();
    let shape = other.scene.add_primitive("box", root, 0).unwrap();
    other.select_only(shape);
    other.open_pattern_tool();
    let target = other.pattern_tool.unwrap();
    // Picked from the question the window opens with, which it answers.
    assert!(!other.pattern_tool_started);
    let scattered = other.pattern_kinds.iter().find(|e| e.name == "Scattered").cloned().unwrap();
    other.apply_saved_kind_to(target, &scattered);
    assert!(other.pattern_tool_started, "picking a saved kind did not answer the question");
    assert_eq!(params(&other, target).num("noise_x"), 0.7, "the kind came back without its noise");

    other.scene.get_mut(target).and_then(|n| n.params_mut()).unwrap().insert("noise_y".into(), ParamValue::Length(2.0));
    let bare = other.pattern_kinds.iter().find(|e| e.name == "Bare").cloned().unwrap();
    other.apply_saved_kind_to(target, &bare);
    assert_eq!(params(&other, target).num("noise_y"), 2.0, "a kind saved without noise took the pattern's off");
}

/// The pointer over a stage marks what the rule has made by the end of it.
#[test]
pub(crate) fn the_copies_through_a_stage_are_where_the_viewport_marks_them() {
    let (mut app, pattern) = with_rule();
    app.add_stage_doing(pattern::StageMode::Move);
    app.reevaluate_for_test();
    let rule = params(&app, pattern);
    assert_eq!(app.pattern_placements_through(0).len(), pattern::stage(&rule, 0).copies());
    assert_eq!(app.pattern_placements_through(1).len(), pattern::instance_count(&rule).1);
}

/// The window fits whatever width it is given with everything a stage can
/// show open at once -- the arrows and cross on every heading, a card for each
/// of several variations, a shift's three fields on one row, the noise card --
/// and so does the question, with the ready-made layouts and a saved kind on it.
#[test]
pub(crate) fn the_tool_fits_its_width_with_every_stage_section_open() {
    let dir = temp_config_dir("pattern-tool-fit-vary");
    let (mut app, pattern) = {
        let mut app = app_in(dir);
        app.open_pattern_tool();
        let pattern = app.pattern_tool.unwrap();
        (app, pattern)
    };
    app.pattern_tool_name = "A kind with rather a long name".into();
    app.start_rule_from(pattern, 0);
    app.save_current_kind();
    app.start_rule_from_preset(pattern, 0);
    app.add_stage_doing(pattern::StageMode::Move);
    app.add_stage_doing(pattern::StageMode::Move);
    {
        let params = app.scene.get_mut(pattern).and_then(|n| n.params_mut()).unwrap();
        pattern::add_variation(params, 0, pattern::Variation::spin(2, 5.0));
        pattern::add_variation(params, 0, pattern::Variation::resize(pattern::ALL_AXES, 0.95));
        pattern::add_variation(params, 0, pattern::Variation::shift(0, 1.0).repeating(3).reaching(3, 1));
        pattern::add_variation(params, 0, pattern::Variation::shift(2, 2.0).repeating(3).reaching(3, 1));
        params.insert("noise_turn_z".into(), ParamValue::Angle(4.0));
        params.insert("noise_turn_x".into(), ParamValue::Angle(2.0));
    }
    app.sync_pattern_tool_sections();
    app.reevaluate_for_test();

    for asking in [false, true] {
        if asking {
            app.start_rule_over();
        }
        for width in [900.0_f32, 470.0, 360.0, 320.0, 300.0, 280.0] {
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 2400.0))),
                ..Default::default()
            };
            let (mut room, mut used) = (0.0_f32, 0.0_f32);
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    room = ui.max_rect().width();
                    crate::pattern_tool::body(&mut app, ui);
                    used = ui.min_rect().width();
                });
            });
            let what = if asking { "the question" } else { "the stages" };
            assert!(used <= room + 0.5, "{what} at {room} px of room laid out {used} px of content");
            if !asking {
                // Every stage drew its own cross, flush with its own fields.
                for index in 0..3 {
                    assert!(ctx.read_response(crate::pattern_tool::drop_stage_id(index)).is_some());
                }
                for grip in
                    ["tool:1.1 Spin", "tool:1.4 Shift", "tool:Jitter X", "tool:Jitter turn Z", "tool:Jitter turn X"]
                {
                    assert!(
                        ctx.read_response(crate::panel_properties::grip_id(grip)).is_some(),
                        "the builder did not draw {grip}"
                    );
                }
            }
        }
    }
}
