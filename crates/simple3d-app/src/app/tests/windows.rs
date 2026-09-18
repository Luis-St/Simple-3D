//! Where an opened model goes, and what a tab drag would do with the tab
//! (issue 107).
//!
//! The moving itself is the shell's and is tested there; what is checked here is
//! the half a single window answers -- the setting that says where a model
//! opens, and the arithmetic that turns a released drag into one of three
//! outcomes.

use super::*;
use crate::shell::{OtherWindow, WindowRequest};
use crate::tabs::drag::{drop_of, TabDrop, PULL_OUT};
use simple3d_core::config::OpenTarget;
use simple3d_core::project;
use simple3d_core::scene::Scene;

/// With "a window of its own" chosen, opening a model asks for a window rather
/// than quietly taking a tab -- and the window it was opened from keeps what it
/// had.
#[test]
fn opening_a_model_asks_for_a_window_when_the_setting_says_so() {
    let dir = temp_config_dir("open-target-window");
    let mut app = app_in(dir.clone());
    app.settings.open_target = OpenTarget::Window;
    let first = dir.join("first.simple3d");
    app.save_to(&first);

    let second = dir.join("second.simple3d");
    std::fs::write(&second, project::to_string(&Scene::new())).unwrap();
    app.open_path(&second);

    assert_eq!(app.window_request, Some(WindowRequest::Open(second)), "the file did not ask for a window");
    assert_eq!(app.tab_count(), 1, "it took a tab as well");
    assert_eq!(app.path.as_deref(), Some(first.as_path()), "the document on screen was replaced");
}

/// The same setting, but with nothing in the window worth keeping: an untouched,
/// never-saved document is scratch space, so the file opens in it rather than
/// leaving an empty window behind.
#[test]
fn a_model_opened_into_an_untouched_window_stays_in_it() {
    let dir = temp_config_dir("open-target-scratch");
    let mut app = app_in(dir.clone());
    app.settings.open_target = OpenTarget::Window;

    let path = dir.join("only.simple3d");
    std::fs::write(&path, project::to_string(&Scene::new())).unwrap();
    app.open_path(&path);

    assert_eq!(app.window_request, None, "an empty window asked for a second one");
    assert_eq!(app.tab_count(), 1);
    assert_eq!(app.path.as_deref(), Some(path.as_path()));
}

/// A file that is already open in *this* window is shown rather than opened
/// again, whatever the setting says -- the older rule still holds.
#[test]
fn a_file_open_in_this_window_is_shown_rather_than_given_a_window() {
    let dir = temp_config_dir("open-target-already");
    let mut app = app_in(dir.clone());
    app.settings.open_target = OpenTarget::Window;
    let first = dir.join("first.simple3d");
    app.save_to(&first);
    app.new_project();

    app.open_path(&first);

    assert_eq!(app.window_request, None, "a file that was already open asked for a window");
    assert_eq!(app.active, 0, "it did not show the tab the file was already in");
}

// -- what a released drag does ------------------------------------------------

fn strip() -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(0.0, 40.0), egui::vec2(900.0, 26.0))
}

/// The window the row is in: the whole of what this window draws.
fn contents() -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0))
}

/// Along the row: the tabs are left as they are. A drag has to mean something
/// before it does something, and sliding along the row -- or overshooting it on
/// the way to the close cross -- means nothing.
#[test]
fn a_drag_that_stays_on_the_row_leaves_the_tabs_alone() {
    let strip = strip();
    let along = egui::pos2(400.0, strip.center().y);
    assert_eq!(drop_of(along, strip, contents(), None, &[], false), TabDrop::Stay);
    let just_below = egui::pos2(400.0, strip.bottom() + PULL_OUT - 1.0);
    assert_eq!(
        drop_of(just_below, strip, contents(), None, &[], false),
        TabDrop::Stay,
        "a near miss pulled the tab out"
    );
}

/// Off the row: a window of its own. It is the one gesture that needs nothing of
/// the window system -- which is why it is the one that works everywhere.
#[test]
fn a_tab_pulled_off_the_row_opens_in_a_window_of_its_own() {
    let strip = strip();
    let below = egui::pos2(400.0, strip.bottom() + PULL_OUT + 1.0);
    assert_eq!(drop_of(below, strip, contents(), None, &[], false), TabDrop::NewWindow);
    let above = egui::pos2(400.0, strip.top() - PULL_OUT - 1.0);
    assert_eq!(drop_of(above, strip, contents(), None, &[], false), TabDrop::NewWindow);
}

/// Onto another window's row: into that window. The pointer is in this window's
/// coordinates and the other window's row is on the desktop, so the two are only
/// comparable once the window system has said where this window is.
#[test]
fn a_tab_dropped_on_another_windows_row_goes_into_that_window() {
    let strip = strip();
    let other = OtherWindow {
        id: 7,
        name: "other.simple3d".into(),
        strip: Some(egui::Rect::from_min_size(egui::pos2(1000.0, 220.0), egui::vec2(900.0, 26.0))),
    };
    // This window's contents start at (60, 40) on the desktop, so a pointer at
    // (1000, 200) in this window is at (1060, 240) on it -- which is on the
    // other window's row.
    let origin = Some(egui::pos2(60.0, 40.0));
    let onto = egui::pos2(1000.0, 200.0);
    assert_eq!(drop_of(onto, strip, contents(), origin, std::slice::from_ref(&other), false), TabDrop::Into(7));
    // The same pointer, but with no idea where this window is: nothing can be
    // aimed at from here, so the tab is held out instead of being dropped
    // somewhere it may not have been let go over. Which window takes it is then
    // the windows' own answer -- see `Shell::resolve_offer`.
    assert_eq!(drop_of(onto, strip, contents(), None, std::slice::from_ref(&other), false), TabDrop::Offer);
}

/// Held out only when there is somewhere for it to go. The last window open has
/// no one to offer a tab to, so letting it go outside the window means what it
/// has always meant: a window of its own, with no wait for an answer.
#[test]
fn a_tab_let_go_outside_the_only_window_opens_at_once() {
    let strip = strip();
    let outside = egui::pos2(1000.0, 200.0);
    assert_eq!(drop_of(outside, strip, contents(), None, &[], false), TabDrop::NewWindow);
}

/// And where the window system does say where windows are, nothing is held out
/// at all: the pointer's place on the desktop settles it on the spot.
#[test]
fn a_tab_let_go_over_the_desktop_opens_in_a_window_of_its_own() {
    let strip = strip();
    let other = OtherWindow {
        id: 7,
        name: "other.simple3d".into(),
        strip: Some(egui::Rect::from_min_size(egui::pos2(1000.0, 220.0), egui::vec2(900.0, 26.0))),
    };
    let origin = Some(egui::pos2(60.0, 40.0));
    // Outside this window, and nowhere near the other window's row.
    let nowhere = egui::pos2(1000.0, 600.0);
    assert_eq!(drop_of(nowhere, strip, contents(), origin, std::slice::from_ref(&other), false), TabDrop::NewWindow);
}

/// A whole window is only ever carried *into* another window: there is nowhere
/// else to put one, and letting go of it over the desktop leaves it where it is.
#[test]
fn carrying_a_whole_window_off_the_row_does_nothing() {
    let strip = strip();
    let below = egui::pos2(400.0, strip.bottom() + PULL_OUT + 40.0);
    assert_eq!(drop_of(below, strip, contents(), None, &[], true), TabDrop::Stay);
}

/// Windows overlap, and two windows of one application overlap along their rows
/// of tabs above all. A drag that never left this window is about this window,
/// whatever is stacked underneath it.
#[test]
fn a_window_underneath_this_one_is_not_dropped_on_by_a_drag_that_never_left() {
    let strip = strip();
    let under = OtherWindow {
        id: 9,
        name: "underneath.simple3d".into(),
        strip: Some(egui::Rect::from_min_size(egui::pos2(0.0, 40.0), egui::vec2(900.0, 26.0))),
    };
    // Exactly over the other window's row, and still inside this window: the
    // pointer never left, so the tab stays where it is.
    let along = egui::pos2(400.0, strip.center().y);
    let origin = Some(egui::Pos2::ZERO);
    assert_eq!(drop_of(along, strip, contents(), origin, std::slice::from_ref(&under), false), TabDrop::Stay);
}
