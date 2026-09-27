//! Every panel and dialog draws, in every state and every dock.

use super::*;
use simple3d_core::keymap::Command;

#[test]
pub(crate) fn every_panel_draws_with_a_selection_and_with_none() {
    // With nothing selected the right dock shows document settings, a path no other test runs.
    let mut app = headless_app();
    let id = app.scene.depth_first().into_iter().find(|&id| id != app.scene.root()).unwrap();
    app.select_only(id);
    draw_one_frame(&mut app);
    app.clear_selection();
    draw_one_frame(&mut app);

    // A group selection reaches the boolean panel, a third layout.
    app.select_only(id);
    app.run(Command::Group);
    draw_one_frame(&mut app);
}

#[test]
pub(crate) fn every_modal_draws_and_so_does_a_palette_with_saved_primitives_on_it() {
    // A panicking window or a zero-width layout fails here rather than in front of someone.
    let dir = temp_config_dir("modals");
    let mut app = app_in(dir);
    let root = app.scene.root();
    let plate = app.scene.add_primitive("plate", root, 0).unwrap();
    app.select_only(plate);

    // Saving gives the palette a Saved section and the naming window something to name.
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
    // Movable panels reach new layouts (an empty dock, three in a column, all rolled up) that must draw.
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
