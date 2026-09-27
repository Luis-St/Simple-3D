//! Several documents open at once.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::project;
use simple3d_core::scene::{GroupOp, Scene};

/// Each tab has its own model, selection, history and camera (issue 61).
#[test]
pub(crate) fn each_tab_keeps_its_own_document() {
    let mut app = app_in(temp_config_dir("tabs-own-document"));
    let root = app.scene.root();
    let plate = app.scene.add_primitive("plate", root, 0).unwrap();
    app.select_only(plate);
    app.scene.camera.distance = 321.0;

    app.run(Command::New);
    assert_eq!(app.tab_count(), 2, "File > New did not open a second document");
    assert_eq!(app.active, 1);
    assert_eq!(app.scene.depth_first(), vec![app.scene.root()], "the new tab opened on the other tab's model");
    assert!(app.selection.is_empty(), "the new tab inherited a selection");

    let second_root = app.scene.root();
    let cube = app.scene.add_primitive("box", second_root, 0).unwrap();
    app.select_only(cube);

    app.activate_tab(0);
    assert_eq!(app.selection, vec![plate], "the first document lost its selection while it was away");
    assert_eq!(app.scene.camera.distance, 321.0, "the first document lost its camera while it was away");
    assert_eq!(app.scene.depth_first().len(), 2, "the first document lost its model while it was away");

    app.cycle_tab(1);
    assert_eq!(app.active, 1);
    assert_eq!(app.selection, vec![cube], "the second document lost its selection while it was away");
    app.cycle_tab(1);
    assert_eq!(app.active, 0, "walking off the end of the row did not wrap");
}

/// Opening a file reuses an untouched document, and an open file is shown rather than reopened
/// (issue 61).
#[test]
pub(crate) fn opening_a_project_reuses_a_scratch_tab_and_never_opens_one_file_twice() {
    let dir = temp_config_dir("tabs-open");
    let mut app = app_in(dir.clone());
    let root = app.scene.root();
    app.scene.add_primitive("plate", root, 0).unwrap();
    let first = dir.join("first.simple3d");
    app.save_to(&first);

    let second = dir.join("second.simple3d");
    std::fs::write(&second, project::to_string(&Scene::new())).unwrap();

    // The saved document is not scratch, so the second file gets its own tab.
    app.open_path(&second);
    assert_eq!(app.tab_count(), 2);
    assert_eq!(app.path.as_deref(), Some(second.as_path()));

    // Both are open, so neither opens again.
    app.open_path(&first);
    assert_eq!(app.tab_count(), 2, "a file that was already open opened a second time");
    assert_eq!(app.active, 0);
    app.open_path(&second);
    assert_eq!(app.tab_count(), 2);
    assert_eq!(app.active, 1);

    // A new untouched document is scratch, so a file opens into it.
    app.run(Command::New);
    assert_eq!(app.tab_count(), 3);
    let third = dir.join("third.simple3d");
    std::fs::write(&third, project::to_string(&Scene::new())).unwrap();
    app.open_path(&third);
    assert_eq!(app.tab_count(), 3, "an empty, never-saved document was left behind as its own tab");
    assert_eq!(app.path.as_deref(), Some(third.as_path()));
}

/// Closing asks before discarding work, and the last document empties rather than closes (issue 61).
#[test]
pub(crate) fn closing_a_tab_asks_about_changes_and_the_last_one_empties_instead_of_vanishing() {
    let mut app = app_in(temp_config_dir("tabs-close"));
    app.run(Command::New);
    app.add_node(Some("plate"), GroupOp::Union);
    assert!(app.unsaved());

    app.run(Command::CloseTab);
    assert_eq!(app.modal, Modal::ConfirmCloseTab, "closing a modified document asked nothing");
    assert_eq!(app.tab_count(), 2, "the document closed before the question was answered");
    app.cancel_close_tab();
    assert_eq!(app.modal, Modal::None);
    assert_eq!(app.tab_count(), 2, "cancelling the question closed the document anyway");

    app.run(Command::CloseTab);
    app.confirm_close_tab();
    assert_eq!(app.tab_count(), 1);
    assert_eq!(app.active, 0);
    assert_eq!(app.modal, Modal::None);

    // Closing the only document leaves an empty one.
    app.add_node(Some("plate"), GroupOp::Union);
    app.close_tab_now(0);
    assert_eq!(app.tab_count(), 1, "the last document closed and left no document at all");
    assert_eq!(app.scene.depth_first(), vec![app.scene.root()]);
    assert!(!app.unsaved(), "the emptied document counts as modified");
}

/// Quitting asks about every open document, including hidden tabs (issue 61).
#[test]
pub(crate) fn quitting_asks_about_a_document_that_is_not_on_screen() {
    let mut app = app_in(temp_config_dir("tabs-quit"));
    app.add_node(Some("plate"), GroupOp::Union);
    assert!(app.unsaved());
    app.run(Command::New);
    assert!(!app.unsaved(), "the new document is untouched");
    assert!(app.any_unsaved(), "the changes in the other tab were not noticed");

    app.request_quit();
    assert_eq!(app.modal, Modal::ConfirmQuit);
    assert!(!app.quit_now, "quitting went ahead with unsaved changes in another tab");
}

/// The tab row and the close question both draw.
#[test]
pub(crate) fn a_row_of_tabs_draws_and_so_does_the_question_a_close_asks() {
    let mut app = headless_app();
    app.run(Command::New);
    app.run(Command::New);
    draw_one_frame(&mut app);
    app.activate_tab(0);
    draw_one_frame(&mut app);
    app.pending_close = Some(0);
    app.modal = Modal::ConfirmCloseTab;
    draw_one_frame(&mut app);
}
