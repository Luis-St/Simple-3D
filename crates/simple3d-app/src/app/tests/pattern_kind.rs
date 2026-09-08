//! Saving a custom pattern rule and using it again.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::primitive::{ParamValue, ParamsExt};

/// The custom pattern kind creation tool, end to end (issue 67): it makes a
/// pattern out of what is selected, builds a rule that no fixed kind can
/// say, keeps it under a name, and puts it back on a different pattern in a
/// different project.
#[test]
pub(crate) fn a_custom_pattern_kind_can_be_built_saved_and_used_again() {
    let dir = temp_config_dir("pattern-kind");
    let mut app = app_in(dir.clone());
    let root = app.scene.root();
    let shape = app.scene.add_primitive("box", root, 0).unwrap();
    app.select_only(shape);

    // Opening the tool with a shape selected wraps it, which is what makes
    // this a creation tool rather than an editor of something already there.
    app.open_pattern_tool();
    let pattern = app.pattern_tool.expect("the tool should have a pattern to work on");
    assert!(app.scene.node(pattern).is_pattern());
    assert_eq!(app.scene.node(pattern).params().unwrap().int("kind"), simple3d_core::pattern::CUSTOM);
    assert_eq!(app.modal, Modal::PatternKind);
    draw_one_frame(&mut app);

    // A row of three, turned four times about Z: two stages, and twelve
    // copies that no single fixed kind lays out.
    let mut stage = simple3d_core::pattern::stage(app.scene.node(pattern).params().unwrap(), 1);
    stage.count = 4;
    stage.turn = 90.0;
    stage.step = simple3d_geom::Vec3::ZERO;
    {
        let params = app.scene.get_mut(pattern).and_then(|n| n.params_mut()).unwrap();
        params.insert("stages".to_string(), simple3d_core::primitive::ParamValue::Count(2));
        params.insert("stage1_count".to_string(), ParamValue::Count(3));
        simple3d_core::pattern::set_stage(params, 1, stage);
    }
    let params = app.scene.node(pattern).params().unwrap().clone();
    assert_eq!(simple3d_core::pattern::instance_count(&params).1, 12);
    draw_one_frame(&mut app);

    app.pattern_tool_name = "Turned row".to_string();
    app.save_current_kind();
    assert_eq!(app.pattern_kinds.iter().map(|e| e.name.clone()).collect::<Vec<_>>(), vec!["Turned row"]);

    // A second, unrelated pattern in a fresh document takes the same rule.
    let mut other = app_in(dir);
    let root = other.scene.root();
    let shape = other.scene.add_primitive("cylinder", root, 0).unwrap();
    other.select_only(shape);
    other.open_pattern_tool();
    let saved = other.pattern_kinds.first().cloned().expect("the shelf should have the saved kind");
    other.apply_saved_kind(&saved);
    let applied = other.pattern_tool.and_then(|id| other.scene.node(id).params().cloned()).unwrap();
    assert_eq!(applied.int("kind"), simple3d_core::pattern::CUSTOM);
    assert_eq!(simple3d_core::pattern::instance_count(&applied).1, 12, "the saved rule did not come back");
    assert_eq!(other.scene.node(other.pattern_tool.unwrap()).name, "Turned row");

    // And it comes off the shelf again when it is deleted.
    other.delete_saved_kind(&saved);
    assert!(other.pattern_kinds.is_empty());
}

/// The rule the tool builds is an ordinary parameter edit, which is the
/// whole reason it was built out of parameters: undo covers it, and so does
/// saving and reloading the project.
#[test]
pub(crate) fn a_custom_rule_survives_undo_and_a_round_trip_through_the_project_file() {
    let mut app = headless_app();
    let root = app.scene.root();
    let shape = app.scene.add_primitive("box", root, 0).unwrap();
    app.select_only(shape);
    app.open_pattern_tool();
    let pattern = app.pattern_tool.unwrap();

    app.edit("Pattern stages", None);
    {
        let params = app.scene.get_mut(pattern).and_then(|n| n.params_mut()).unwrap();
        params.insert("stages".to_string(), ParamValue::Count(3));
    }
    assert_eq!(app.scene.node(pattern).params().unwrap().int("stages"), 3);
    app.run(Command::Undo);
    assert_eq!(app.scene.node(pattern).params().unwrap().int("stages"), 1, "undo did not reach the stage count");
    app.run(Command::Redo);
    assert_eq!(app.scene.node(pattern).params().unwrap().int("stages"), 3);

    let text = simple3d_core::project::to_string(&app.scene);
    let back = simple3d_core::project::from_str(&text).expect("the project should reload");
    let reloaded = back
        .depth_first()
        .into_iter()
        .find(|id| back.node(*id).is_pattern())
        .and_then(|id| back.node(id).params().cloned())
        .expect("the pattern should have survived the file");
    assert_eq!(reloaded.int("kind"), simple3d_core::pattern::CUSTOM);
    assert_eq!(reloaded.int("stages"), 3);
}
