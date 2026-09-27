//! Documents moving between windows (issue 107).
//!
//! Tests drive the shell headlessly: a window leaves a [`WindowRequest`] and `resolve_requests`
//! carries it out; only where documents end up and which windows remain is checked.

use super::*;
use crate::tabs::Document;
use std::path::{Path, PathBuf};

fn config_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "simple3d-shell-test-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

impl Shell {
    /// A shell using `dir` for settings, like `App::with_config_dir`.
    fn in_config_dir(ctx: &egui::Context, dir: PathBuf) -> Shell {
        let first = App::with_config_dir(ctx, None, dir);
        let settings = first.settings.clone();
        Shell { windows: vec![first], next_id: 1, gl: None, offer: None, settings }
    }
}

/// A shell with one window holding two saved documents.
fn shell_with_two_documents(name: &str) -> (Shell, PathBuf, egui::Context) {
    let dir = config_dir(name);
    let ctx = egui::Context::default();
    let mut shell = Shell::in_config_dir(&ctx, dir.clone());
    shell.windows[0].save_to(&dir.join("first.simple3d"));
    shell.windows[0].new_project();
    shell.windows[0].save_to(&dir.join("second.simple3d"));
    assert_eq!(shell.windows[0].tab_count(), 2);
    (shell, dir, ctx)
}

/// What is open in a window, tab by tab.
fn open_in(window: &App) -> Vec<String> {
    (0..window.tab_count()).map(|index| window.tab_summary(index).0).collect()
}

#[test]
fn a_tab_taken_out_of_a_window_opens_in_one_of_its_own() {
    let (mut shell, _dir, ctx) = shell_with_two_documents("detach");
    shell.windows[0].window_request = Some(WindowRequest::Detach(0));
    shell.resolve_requests(&ctx);

    assert_eq!(shell.windows.len(), 2, "the tab did not become a window");
    assert_eq!(open_in(&shell.windows[0]), vec!["second.simple3d"], "the wrong document was left behind");
    assert_eq!(open_in(&shell.windows[1]), vec!["first.simple3d"], "the wrong document moved");
    assert_ne!(shell.windows[0].window_id, shell.windows[1].window_id, "two windows share one id");
    // A window opened for one document opens on it, not beside an empty tab.
    assert_eq!(shell.windows[1].path.as_deref().map(Path::to_path_buf), shell.windows[1].tabs_path());
}

/// Pulling out a window's only tab does nothing rather than opening an empty window.
#[test]
fn the_only_tab_of_a_window_cannot_be_pulled_out_of_it() {
    let dir = config_dir("detach-only");
    let ctx = egui::Context::default();
    let mut shell = Shell::in_config_dir(&ctx, dir);
    shell.windows[0].window_request = Some(WindowRequest::Detach(0));
    shell.resolve_requests(&ctx);
    assert_eq!(shell.windows.len(), 1, "a window opened for a document that was already in one");
    assert_eq!(shell.windows[0].tab_count(), 1);
}

#[test]
fn a_tab_dropped_on_another_window_moves_there() {
    let (mut shell, dir, ctx) = shell_with_two_documents("move-tab");
    shell.windows[0].window_request = Some(WindowRequest::Detach(0));
    shell.resolve_requests(&ctx);
    // A third document, so the source window is not emptied by the move.
    shell.windows[1].new_project();
    shell.windows[1].save_to(&dir.join("third.simple3d"));
    let first = shell.windows[0].window_id;

    shell.windows[1].window_request = Some(WindowRequest::MoveTab(0, first));
    shell.resolve_requests(&ctx);

    assert_eq!(shell.windows.len(), 2, "a window was closed although it still had a document in it");
    assert_eq!(open_in(&shell.windows[0]), vec!["second.simple3d", "first.simple3d"]);
    assert_eq!(open_in(&shell.windows[1]), vec!["third.simple3d"]);
}

/// The source window closes when its last document moves out, which also makes dropping a
/// one-tab window's tab onto another read as merging.
#[test]
fn moving_the_last_tab_out_of_a_window_closes_the_window() {
    let (mut shell, _dir, ctx) = shell_with_two_documents("move-last");
    shell.windows[0].window_request = Some(WindowRequest::Detach(0));
    shell.resolve_requests(&ctx);
    let first = shell.windows[0].window_id;
    let moved = shell.windows[1].window_id;

    shell.windows[1].window_request = Some(WindowRequest::MoveTab(0, first));
    shell.resolve_requests(&ctx);

    assert_eq!(shell.windows.len(), 1, "the window the last document left was not closed");
    assert_eq!(shell.windows[0].window_id, first);
    assert_ne!(shell.windows[0].window_id, moved);
    assert_eq!(open_in(&shell.windows[0]), vec!["second.simple3d", "first.simple3d"]);
}

/// Dropping a whole window on another's tab row moves every document in order and closes it.
#[test]
fn every_tab_of_a_window_can_be_moved_into_another_window() {
    let (mut shell, dir, ctx) = shell_with_two_documents("move-all");
    shell.windows[0].window_request = Some(WindowRequest::Detach(0));
    shell.resolve_requests(&ctx);
    shell.windows[1].new_project();
    shell.windows[1].save_to(&dir.join("third.simple3d"));
    let first = shell.windows[0].window_id;

    shell.windows[1].window_request = Some(WindowRequest::MoveAll(first));
    shell.resolve_requests(&ctx);

    assert_eq!(shell.windows.len(), 1);
    assert_eq!(
        open_in(&shell.windows[0]),
        vec!["second.simple3d", "first.simple3d", "third.simple3d"],
        "the documents arrived in the wrong order, or not at all"
    );
}

/// Closing the first (root viewport) window: it leaves the shell and the next one takes its place.
#[test]
fn closing_the_first_window_leaves_the_others_running() {
    let (mut shell, _dir, ctx) = shell_with_two_documents("close-first");
    shell.windows[0].window_request = Some(WindowRequest::Detach(0));
    shell.resolve_requests(&ctx);
    let second = shell.windows[1].window_id;

    shell.windows[0].window_request = Some(WindowRequest::Close);
    shell.resolve_requests(&ctx);

    assert_eq!(shell.windows.len(), 1);
    assert_eq!(shell.windows[0].window_id, second, "the window that was closed is the one still open");
    assert_eq!(open_in(&shell.windows[0]), vec!["first.simple3d"]);
}

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

impl App {
    /// The on-screen document's path, read via its tab, which must agree with the application.
    fn tabs_path(&self) -> Option<PathBuf> {
        let (name, _) = self.tab_summary(self.active);
        self.path.as_ref().filter(|path| path.file_name().is_some_and(|file| file.to_string_lossy() == name)).cloned()
    }
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

/// A tab released outside its window, where window positions are unknown, is held until the
/// window whose tab row has the pointer claims it (issue 107).
#[test]
fn a_tab_held_out_goes_to_the_window_the_pointer_is_over() {
    let (mut shell, dir, ctx) = shell_with_two_documents("offer-claimed");
    shell.windows[0].window_request = Some(WindowRequest::Detach(0));
    shell.resolve_requests(&ctx);
    shell.windows[1].new_project();
    shell.windows[1].save_to(&dir.join("third.simple3d"));
    let first = shell.windows[0].window_id;

    shell.windows[1].window_request = Some(WindowRequest::Offer(Some(0)));
    shell.resolve_requests(&ctx);
    assert!(shell.offer.is_some(), "the tab was not held out");
    shell.resolve_offer(&ctx);
    assert_eq!(shell.windows.len(), 2);
    assert_eq!(open_in(&shell.windows[1]).len(), 2, "the tab left the window before anything claimed it");

    shell.windows[0].pointer_on_strip = true;
    shell.resolve_offer(&ctx);

    assert!(shell.offer.is_none());
    assert_eq!(shell.windows.len(), 2);
    assert_eq!(open_in(&shell.windows[0]), vec!["second.simple3d", "first.simple3d"]);
    assert_eq!(open_in(&shell.windows[1]), vec!["third.simple3d"]);
    assert_eq!(shell.windows[0].window_id, first);
}

/// Released over nothing, a held tab becomes its own window after the claim delay.
#[test]
fn a_tab_nobody_claims_becomes_a_window_of_its_own() {
    let (mut shell, _dir, ctx) = shell_with_two_documents("offer-unclaimed");
    shell.windows[0].window_request = Some(WindowRequest::Offer(Some(0)));
    shell.resolve_requests(&ctx);
    shell.resolve_offer(&ctx);
    assert_eq!(shell.windows.len(), 1, "it was given away with nothing to give it to");

    shell.offer.as_mut().expect("still held out").since = std::time::Instant::now() - CLAIM * 2;
    shell.resolve_offer(&ctx);

    assert!(shell.offer.is_none());
    assert_eq!(shell.windows.len(), 2);
    assert_eq!(open_in(&shell.windows[1]), vec!["first.simple3d"]);
}

/// The source window never claims its own tab back.
#[test]
fn the_window_a_tab_came_from_does_not_claim_it() {
    let (mut shell, _dir, ctx) = shell_with_two_documents("offer-self");
    shell.windows[0].pointer_on_strip = true;
    shell.windows[0].window_request = Some(WindowRequest::Offer(Some(0)));
    shell.resolve_requests(&ctx);
    shell.resolve_offer(&ctx);

    assert!(shell.offer.is_some(), "the window it came from took it straight back");
    assert_eq!(shell.windows.len(), 1);
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
