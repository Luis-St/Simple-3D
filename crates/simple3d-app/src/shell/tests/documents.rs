//! Opening documents into windows, and what a document keeps and where it lands when it moves.

use super::*;

/// A file open in another window is shown there rather than opened twice, which would let the
/// last save silently win.
#[test]
fn a_file_that_is_already_open_somewhere_is_shown_rather_than_opened_again() {
    let (mut shell, dir, ctx) = shell_with_two_documents("open-twice");
    shell.windows[0].window_request = Some(WindowRequest::Detach(0));
    shell.resolve_requests(&ctx);
    assert_eq!(shell.windows.len(), 2);

    shell.windows[0].window_request = Some(WindowRequest::Open(dir.join("first.simple3d")));
    shell.resolve_requests(&ctx);

    assert_eq!(shell.windows.len(), 2, "a file that was already open in another window opened a third time");
    assert_eq!(open_in(&shell.windows[1]), vec!["first.simple3d"]);
}

/// Opening a model into its own window (issue 107): read in the new window, the old one unchanged.
#[test]
fn opening_a_model_in_a_window_of_its_own_reads_it_there() {
    let (mut shell, dir, ctx) = shell_with_two_documents("open-window");
    let third = dir.join("third.simple3d");
    std::fs::write(&third, simple3d_core::project::to_string(&simple3d_core::scene::Scene::new())).unwrap();

    shell.windows[0].window_request = Some(WindowRequest::Open(third));
    shell.resolve_requests(&ctx);

    assert_eq!(shell.windows.len(), 2);
    assert_eq!(open_in(&shell.windows[0]), vec!["first.simple3d", "second.simple3d"]);
    assert_eq!(open_in(&shell.windows[1]), vec!["third.simple3d"], "the new window did not open on the file");
}

/// With embedded dialogs there are no viewports for more windows, so documents fold back into
/// the first one.
#[test]
fn documents_come_back_to_one_window_when_there_is_nowhere_to_put_a_second() {
    let (mut shell, _dir, ctx) = shell_with_two_documents("fold");
    shell.windows[0].window_request = Some(WindowRequest::Detach(0));
    shell.resolve_requests(&ctx);
    assert_eq!(shell.windows.len(), 2);

    shell.settings.embed_dialogs = true;
    shell.fold_windows_together();

    assert_eq!(shell.windows.len(), 1);
    assert_eq!(open_in(&shell.windows[0]), vec!["second.simple3d", "first.simple3d"]);
}

/// A moved document keeps its model, history and path.
#[test]
fn a_document_that_changes_window_keeps_its_model_and_its_history() {
    let (mut shell, _dir, ctx) = shell_with_two_documents("carry");
    shell.windows[0].activate_tab(0);
    let root = shell.windows[0].scene.root();
    shell.windows[0].scene.add_primitive("plate", root, 0).unwrap();
    let scene = shell.windows[0].scene.clone();
    shell.windows[0].history.record(&scene, "Add a plate", None);
    let shapes = shell.windows[0].scene.node(root).children.len();

    shell.windows[0].window_request = Some(WindowRequest::Detach(0));
    shell.resolve_requests(&ctx);

    let moved = &shell.windows[1];
    let root = moved.scene.root();
    assert_eq!(moved.scene.node(root).children.len(), shapes, "the model did not travel with the document");
    assert!(moved.history.can_undo(), "the undo history was left in the window the document came from");
    assert_eq!(moved.tab_summary(moved.active).0, "first.simple3d");
}

/// A moved document always gets its own tab, even over an untouched scratch document
/// (issue 107). Overwriting the scratch document made the move look like data loss.
#[test]
fn a_document_moved_into_an_empty_window_arrives_in_a_tab_of_its_own() {
    let (mut shell, _dir, ctx) = shell_with_two_documents("scratch");
    let index = shell.add_window(&ctx);
    assert_eq!(shell.windows[index].tab_count(), 1);
    let empty = shell.windows[index].window_id;

    shell.windows[0].window_request = Some(WindowRequest::MoveTab(0, empty));
    shell.resolve_requests(&ctx);

    assert_eq!(
        open_in(&shell.windows[index]),
        vec!["Untitled", "first.simple3d"],
        "the document that moved is not a tab of its own, so nothing on screen says it arrived"
    );
    assert_eq!(shell.windows[index].tab_summary(shell.windows[index].active).0, "first.simple3d");
}

/// A window made for one document does overwrite its scratch document, so it has one tab.
#[test]
fn a_window_opened_for_one_document_comes_up_with_one_tab() {
    let (mut shell, _dir, ctx) = shell_with_two_documents("detach-one-tab");
    shell.windows[0].window_request = Some(WindowRequest::Detach(0));
    shell.resolve_requests(&ctx);
    assert_eq!(open_in(&shell.windows[1]), vec!["first.simple3d"]);
}

/// The document left behind is the neighbour, as when a tab is closed.
#[test]
fn taking_a_tab_shows_the_neighbour_of_the_one_that_went() {
    let (mut shell, dir, _ctx) = shell_with_two_documents("neighbour");
    shell.windows[0].new_project();
    shell.windows[0].save_to(&dir.join("third.simple3d"));
    shell.windows[0].activate_tab(1);

    let taken = shell.windows[0].take_tab(1).expect("the tab is there");
    assert_eq!(taken.path.as_deref(), Some(dir.join("second.simple3d").as_path()));
    assert_eq!(shell.windows[0].tab_summary(shell.windows[0].active).0, "third.simple3d");
    assert_eq!(open_in(&shell.windows[0]), vec!["first.simple3d", "third.simple3d"]);
}

/// A window's name in send offers: the on-screen document and how many are behind it.
#[test]
fn a_window_is_named_after_the_document_on_screen() {
    let (mut shell, _dir, _ctx) = shell_with_two_documents("summary");
    assert_eq!(shell.windows[0].window_summary(), "second.simple3d and 1 more");
    shell.windows[0].close_tab_now(0);
    assert_eq!(shell.windows[0].window_summary(), "second.simple3d");
}

/// A document arriving while another is on screen goes into a tab beside it.
#[test]
fn adopting_a_document_never_writes_over_the_one_on_screen() {
    let (mut shell, _dir, _ctx) = shell_with_two_documents("adopt");
    let before = open_in(&shell.windows[0]);
    shell.windows[0].adopt(Document::empty());
    assert_eq!(shell.windows[0].tab_count(), before.len() + 1);
    for name in before {
        assert!(open_in(&shell.windows[0]).contains(&name), "a document that was open is not open any more");
    }
}
