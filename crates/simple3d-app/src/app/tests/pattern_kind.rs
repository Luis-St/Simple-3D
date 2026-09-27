//! Saving a custom pattern rule and using it again.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::primitive::{ParamValue, ParamsExt};

/// The custom kind tool end to end (issue 67): wrap the selection, build a rule no fixed kind can
/// express, save it, and apply it to another pattern in another project.
#[test]
pub(crate) fn a_custom_pattern_kind_can_be_built_saved_and_used_again() {
    let dir = temp_config_dir("pattern-kind");
    let mut app = app_in(dir.clone());
    let root = app.scene.root();
    let shape = app.scene.add_primitive("box", root, 0).unwrap();
    app.select_only(shape);

    // Opening the tool with a shape selected wraps it.
    app.open_pattern_tool();
    let pattern = app.pattern_tool.expect("the tool should have a pattern to work on");
    assert!(app.scene.node(pattern).is_pattern());
    // The window asks what to start from; a run here.
    app.start_rule_from(pattern, 0);
    assert_eq!(app.scene.node(pattern).params().unwrap().int("kind"), simple3d_core::pattern::CUSTOM);
    draw_one_frame(&mut app);

    // A row of three turned four times about Z: twelve copies no fixed kind lays out.
    let mut stage = simple3d_core::pattern::stage(app.scene.node(pattern).params().unwrap(), 1);
    stage.mode = simple3d_core::pattern::StageMode::Turn;
    stage.count = 4;
    stage.turn = 90.0;
    {
        let params = app.scene.get_mut(pattern).and_then(|n| n.params_mut()).unwrap();
        params.insert("stages".to_string(), simple3d_core::primitive::ParamValue::Count(2));
        params.insert("stage1_count".to_string(), ParamValue::Count(3));
        simple3d_core::pattern::set_stage(params, 1, &stage);
    }
    let params = app.scene.node(pattern).params().unwrap().clone();
    assert_eq!(simple3d_core::pattern::instance_count(&params).1, 12);
    draw_one_frame(&mut app);

    app.pattern_tool_name = "Turned row".to_string();
    app.save_current_kind();
    assert_eq!(app.pattern_kinds.iter().map(|e| e.name.clone()).collect::<Vec<_>>(), vec!["Turned row"]);

    // An unrelated pattern in a fresh document takes the same rule.
    let mut other = app_in(dir);
    let root = other.scene.root();
    let shape = other.scene.add_primitive("cylinder", root, 0).unwrap();
    other.select_only(shape);
    other.open_pattern_tool();
    let saved = other.pattern_kinds.first().cloned().expect("the shelf should have the saved kind");
    let target = other.pattern_tool.expect("the tool opened on a pattern");
    other.apply_saved_kind_to(target, &saved);
    let applied = other.pattern_tool.and_then(|id| other.scene.node(id).params().cloned()).unwrap();
    assert_eq!(applied.int("kind"), simple3d_core::pattern::CUSTOM);
    assert_eq!(simple3d_core::pattern::instance_count(&applied).1, 12, "the saved rule did not come back");
    assert_eq!(other.scene.node(other.pattern_tool.unwrap()).name, "Turned row");

    // Deleting it removes it from the shelf.
    other.delete_saved_kind(&saved);
    assert!(other.pattern_kinds.is_empty());
}

/// Deleting a saved kind asks first (issue 67), since it deletes a file undo cannot restore.
#[test]
pub(crate) fn a_saved_kind_is_not_deleted_until_the_question_is_answered() {
    let dir = temp_config_dir("pattern-kind-delete");
    let mut app = app_in(dir);
    let root = app.scene.root();
    let shape = app.scene.add_primitive("box", root, 0).unwrap();
    app.select_only(shape);
    app.open_pattern_tool();
    let pattern = app.pattern_tool.expect("the tool should have a pattern to work on");
    app.start_rule_from(pattern, 0);
    app.pattern_tool_name = "Keep me".to_string();
    app.save_current_kind();
    let saved = app.pattern_kinds.first().cloned().expect("the shelf should have the saved kind");

    // The cross asks; it does not delete.
    app.ask_delete_saved_kind(saved.clone());
    assert_eq!(app.modal, crate::app::Modal::ConfirmDeleteKind, "the cross deleted without asking");
    assert_eq!(app.confirm_delete_kind.as_ref().map(|e| e.name.clone()), Some("Keep me".to_string()));
    assert!(saved.path.exists(), "the file went before the question was answered");

    // "No" keeps the kind.
    app.dismiss_modal();
    assert_eq!(app.modal, crate::app::Modal::None);
    assert!(app.confirm_delete_kind.is_none(), "the question was left holding the kind it asked about");
    assert!(saved.path.exists(), "cancelling the question deleted the kind anyway");
    app.refresh_pattern_kinds();
    assert_eq!(app.pattern_kinds.len(), 1, "cancelling the question took the kind off the shelf");

    // "Yes" removes it and its file.
    app.ask_delete_saved_kind(saved.clone());
    app.delete_saved_kind(&saved);
    app.modal = crate::app::Modal::None;
    app.confirm_delete_kind = None;
    assert!(!saved.path.exists(), "the kind was left on disk");
    assert!(app.pattern_kinds.is_empty(), "the shelf still offers a kind that has been deleted");
}

/// The tool's rule is an ordinary parameter edit, so undo and save/reload cover it.
#[test]
pub(crate) fn a_custom_rule_survives_undo_and_a_round_trip_through_the_project_file() {
    let mut app = headless_app();
    let root = app.scene.root();
    let shape = app.scene.add_primitive("box", root, 0).unwrap();
    app.select_only(shape);
    app.open_pattern_tool();
    let pattern = app.pattern_tool.unwrap();
    app.start_rule_from(pattern, 0);

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
