//! Every panel and dialog draws, in every state and every dock.

use super::*;
use simple3d_core::keymap::Command;

#[test]
pub(crate) fn every_panel_draws_with_a_selection_and_with_none() {
    // The two states put different panels on screen: with nothing selected
    // the right dock swaps the property panels for the document's own
    // settings, and that path has no other test that ever runs it.
    let mut app = headless_app();
    let id = app.scene.depth_first().into_iter().find(|&id| id != app.scene.root()).unwrap();
    app.select_only(id);
    draw_one_frame(&mut app);
    app.clear_selection();
    draw_one_frame(&mut app);

    // A group selection reaches the boolean panel, which is a third layout
    // again.
    app.select_only(id);
    app.run(Command::Group);
    draw_one_frame(&mut app);
}

#[test]
pub(crate) fn every_modal_draws_and_so_does_a_palette_with_saved_primitives_on_it() {
    // A window that panics, or a layout that divides by a width it does not
    // have, fails here rather than in front of someone.
    let dir = temp_config_dir("modals");
    let mut app = app_in(dir);
    let root = app.scene.root();
    let plate = app.scene.add_primitive("plate", root, 0).unwrap();
    app.select_only(plate);

    // Saving one gives the palette a Saved section to draw, and the naming
    // window something to name.
    app.save_selection_as_primitive();
    draw_one_frame(&mut app);
    app.confirm_save_primitive();
    assert_eq!(app.library.len(), 1);
    draw_one_frame(&mut app);

    for modal in [
        Modal::Export,
        Modal::Keymap,
        Modal::About,
        Modal::Error,
        Modal::ConfirmQuit,
        Modal::ConfirmCloseTab,
        Modal::SavePrimitive,
        Modal::PatternKind,
    ] {
        app.modal = modal;
        draw_one_frame(&mut app);
    }
    app.modal = Modal::None;
}

#[test]
pub(crate) fn an_empty_scene_still_draws_every_panel() {
    let mut app = headless_app();
    for id in app.scene.depth_first() {
        if id != app.scene.root() {
            app.scene.remove(id);
        }
    }
    app.clear_selection();
    app.reevaluate_for_test();
    draw_one_frame(&mut app);
}

#[test]
pub(crate) fn every_panel_still_draws_wherever_it_has_been_docked() {
    // Panels are movable, so the layouts that used to be impossible -- a
    // dock with nothing in it, three panels in one column, everything rolled
    // up -- are now reachable and have to draw.
    use simple3d_core::config::{Panel, Side};
    let mut app = headless_app();
    for panel in Panel::ALL {
        app.settings.layout.move_to(panel, Side::Right, 0);
    }
    draw_one_frame(&mut app);
    for panel in Panel::ALL {
        app.settings.layout.toggle_collapsed(panel);
    }
    draw_one_frame(&mut app);
    app.settings.layout.docks_hidden = true;
    draw_one_frame(&mut app);
    app.run(Command::ResetLayout);
    assert_eq!(app.settings.layout, simple3d_core::config::Layout::default());
    draw_one_frame(&mut app);
}
