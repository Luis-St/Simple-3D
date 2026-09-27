//! Painting, and the ghost a hidden node is drawn as.

use super::*;
use simple3d_core::scene::Colour;
use simple3d_core::scene::Visibility;

#[test]
pub(crate) fn painting_remembers_the_colour_and_only_a_ghost_is_drawn_as_one() {
    let mut app = headless_app();
    let id = app.primary().expect("the plate is selected");
    assert!(app.settings.recent_colours.is_empty());

    // Issue 29: a colour used once is offered again.
    app.paint(&[id], Some(Colour([0x2E, 0x9A, 0xFF])), None);
    assert_eq!(app.settings.recent_colours, vec![[0x2E, 0x9A, 0xFF]]);
    app.paint(&[id], Some(Colour([0x77, 0x11, 0x22])), None);
    assert_eq!(app.settings.recent_colours[0], [0x77, 0x11, 0x22]);
    // Clearing paints nothing, so remembers nothing.
    app.paint(&[id], None, None);
    assert_eq!(app.settings.recent_colours.len(), 2);

    // Issue 35: presets are already a click away, so they are not recent.
    let preset = crate::theme::PAINT_PRESETS[6].1;
    app.paint(&[id], Some(Colour([preset.r(), preset.g(), preset.b()])), None);
    assert_eq!(app.settings.recent_colours.len(), 2, "a preset was remembered as a recent colour");
    assert_eq!(app.custom_recent_colours(), vec![[0x77, 0x11, 0x22], [0x2E, 0x9A, 0xFF]]);

    // Issues 35 and 85: a picker drag remembers nothing until the picker closes: one visit, one choice.
    for step in 0..40_u8 {
        app.paint_from_picker(&[id], [0x10 + step, 0x40, 0x90]);
        // The undo coalescing window ages out on slow drags, which once split a visit per pause.
        app.history.close();
    }
    assert_eq!(
        app.custom_recent_colours(),
        vec![[0x77, 0x11, 0x22], [0x2E, 0x9A, 0xFF]],
        "the picker put a colour on the row before it was closed"
    );
    app.picker_closed();
    assert_eq!(
        app.custom_recent_colours(),
        vec![[0x37, 0x40, 0x90], [0x77, 0x11, 0x22], [0x2E, 0x9A, 0xFF]],
        "a single drag through the picker filled the recent row"
    );
    // Closing again is not a second choice.
    app.picker_closed();
    assert_eq!(app.custom_recent_colours().len(), 3);

    // A second visit is a second choice.
    app.paint_from_picker(&[id], [0x01, 0x02, 0x03]);
    app.picker_closed();
    assert_eq!(app.custom_recent_colours()[..2], [[0x01, 0x02, 0x03], [0x37, 0x40, 0x90]]);

    // Issue 85: a shade of a colour on the row takes its slot, coming in and for rows older versions filled.
    app.paint_from_picker(&[id], [0x05, 0x06, 0x07]);
    app.picker_closed();
    assert_eq!(app.custom_recent_colours()[..2], [[0x05, 0x06, 0x07], [0x37, 0x40, 0x90]]);
    app.settings.recent_colours = (0..8).map(|n| [n, n, n]).collect();
    assert_eq!(
        app.custom_recent_colours(),
        vec![[0, 0, 0]],
        "a row an older version filled with shades of one black still shows eight of them"
    );

    // Issue 21: the three states, and which are drawn as ghosts.
    assert!(app.ghosts().is_empty());
    app.scene.get_mut(id).unwrap().set_visibility(Visibility::Hidden);
    assert!(app.ghosts().is_empty(), "a hidden node is gone, not translucent");
    app.scene.get_mut(id).unwrap().set_visibility(Visibility::Ghost);
    assert_eq!(app.ghosts(), vec![id]);
    app.scene.get_mut(id).unwrap().set_visibility(Visibility::Visible);
    assert!(app.ghosts().is_empty());
}
