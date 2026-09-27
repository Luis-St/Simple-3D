//! Where an opened model goes, and what a released tab drag does (issue 107). Moving itself is
//! tested in the shell.

use super::*;
use crate::shell::{OtherWindow, WindowRequest};
use crate::tabs::drag::{drop_of, TabDrop, PULL_OUT};
use simple3d_core::config::OpenTarget;
use simple3d_core::project;
use simple3d_core::scene::Scene;

/// With "own window" chosen, opening a model asks for a window and the current one keeps its tabs.
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

/// An untouched scratch document is reused instead of leaving an empty window behind.
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

/// A file already open in this window is shown rather than opened again.
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

/// This window's whole drawn area.
fn contents() -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0))
}

/// Along the row: nothing happens, including overshooting on the way to the close cross.
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

/// Off the row: its own window, the one gesture that works on every window system.
#[test]
fn a_tab_pulled_off_the_row_opens_in_a_window_of_its_own() {
    let strip = strip();
    let below = egui::pos2(400.0, strip.bottom() + PULL_OUT + 1.0);
    assert_eq!(drop_of(below, strip, contents(), None, &[], false), TabDrop::NewWindow);
    let above = egui::pos2(400.0, strip.top() - PULL_OUT - 1.0);
    assert_eq!(drop_of(above, strip, contents(), None, &[], false), TabDrop::NewWindow);
}

/// Onto another window's row: into that window, once this window's desktop position is known.
#[test]
fn a_tab_dropped_on_another_windows_row_goes_into_that_window() {
    let strip = strip();
    let other = OtherWindow {
        id: 7,
        name: "other.simple3d".into(),
        strip: Some(egui::Rect::from_min_size(egui::pos2(1000.0, 220.0), egui::vec2(900.0, 26.0))),
    };
    // Contents at (60, 40) on the desktop, so (1000, 200) here is (1060, 240) there: the other row.
    let origin = Some(egui::pos2(60.0, 40.0));
    let onto = egui::pos2(1000.0, 200.0);
    assert_eq!(drop_of(onto, strip, contents(), origin, std::slice::from_ref(&other), false), TabDrop::Into(7));
    // With no known position the tab is held out for the windows to claim (`Shell::resolve_offer`).
    assert_eq!(drop_of(onto, strip, contents(), None, std::slice::from_ref(&other), false), TabDrop::Offer);
}

/// Held out only if another window exists; otherwise it becomes its own window at once.
#[test]
fn a_tab_let_go_outside_the_only_window_opens_at_once() {
    let strip = strip();
    let outside = egui::pos2(1000.0, 200.0);
    assert_eq!(drop_of(outside, strip, contents(), None, &[], false), TabDrop::NewWindow);
}

/// With known window positions, nothing is held out: the desktop position decides.
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

/// A whole window only goes into another window; released over the desktop it stays.
#[test]
fn carrying_a_whole_window_off_the_row_does_nothing() {
    let strip = strip();
    let below = egui::pos2(400.0, strip.bottom() + PULL_OUT + 40.0);
    assert_eq!(drop_of(below, strip, contents(), None, &[], true), TabDrop::Stay);
}

/// A drag that never left this window is about this window, whatever overlaps beneath it.
#[test]
fn a_window_underneath_this_one_is_not_dropped_on_by_a_drag_that_never_left() {
    let strip = strip();
    let under = OtherWindow {
        id: 9,
        name: "underneath.simple3d".into(),
        strip: Some(egui::Rect::from_min_size(egui::pos2(0.0, 40.0), egui::vec2(900.0, 26.0))),
    };
    // Over the other window's row but still inside this window: the tab stays.
    let along = egui::pos2(400.0, strip.center().y);
    let origin = Some(egui::Pos2::ZERO);
    assert_eq!(drop_of(along, strip, contents(), origin, std::slice::from_ref(&under), false), TabDrop::Stay);
}
