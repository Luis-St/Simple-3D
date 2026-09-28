//! Documents moving between windows (issue 107).
//!
//! Tests drive the shell headlessly: a window leaves a [`WindowRequest`] and `resolve_requests`
//! carries it out; only where documents end up and which windows remain is checked.

use super::*;
use crate::app::tests::temp_config_dir;
use crate::tabs::Document;
use std::path::{Path, PathBuf};

mod documents;

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
    let dir = temp_config_dir(name);
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

impl App {
    /// The on-screen document's path, read via its tab, which must agree with the application.
    fn tabs_path(&self) -> Option<PathBuf> {
        let (name, _) = self.tab_summary(self.active);
        self.path.as_ref().filter(|path| path.file_name().is_some_and(|file| file.to_string_lossy() == name)).cloned()
    }
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
    let dir = temp_config_dir("detach-only");
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
