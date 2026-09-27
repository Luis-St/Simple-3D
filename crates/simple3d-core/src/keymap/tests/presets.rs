//! What each preset binds, and switching between them.

use super::*;

#[test]
pub(crate) fn every_preset_binds_every_command_without_conflicts() {
    for preset in Preset::ALL {
        let map = Keymap::from_preset(preset);
        assert!(map.self_conflicts().is_empty(), "{preset:?}: {:?}", map.self_conflicts());
        for command in Command::ALL {
            assert!(map.binding(*command).is_some(), "{preset:?} does not bind {:?}", command);
        }
    }
}

#[test]
pub(crate) fn every_command_has_a_label_and_an_area() {
    for command in Command::ALL {
        assert!(!command.label().is_empty(), "{command:?}");
        assert!(Area::ALL.contains(&command.area()));
    }
    // Every area has at least one command.
    for area in Area::ALL {
        assert!(Command::ALL.iter().any(|c| c.area() == area), "{area:?} is empty");
    }
}

#[test]
pub(crate) fn switching_preset_changes_navigation_and_mode_keys() {
    // Spec section 8.2, acceptance criterion 27.
    let default = Keymap::from_preset(Preset::Default);
    let mesh = Keymap::from_preset(Preset::MeshEditor);
    let cad = Keymap::from_preset(Preset::Cad);
    assert_ne!(default.nav.orbit, mesh.nav.orbit);
    assert_ne!(mesh.nav.pan, cad.nav.pan);
    assert!(cad.nav.invert_zoom);
    assert_eq!(mesh.binding(Command::ModeMove), Some(&Chord::key("G")));
    assert_eq!(default.binding(Command::ModeMove), Some(&Chord::key("W")));
}

#[test]
pub(crate) fn a_default_that_has_moved_is_carried_over_but_a_chosen_binding_is_not() {
    // Issue 77: snap moved from V to Ctrl, and saved keymaps carrying the old default must move with it.
    let stored = r#"{
      "preset": "default",
      "bindings": { "snap_to_geometry": "V" },
      "nav": { "orbit": { "button": "right" }, "pan": { "button": "right", "shift": true },
               "invert_zoom": false }
    }"#;
    let map = Keymap::from_text(stored).unwrap();
    assert_eq!(map.binding(Command::SnapToGeometry), Some(&Chord::modifiers(true, false, false)));

    // A binding the user changed is theirs.
    let chosen = stored.replace("\"V\"", "\"Shift+K\"");
    let map = Keymap::from_text(&chosen).unwrap();
    assert_eq!(map.binding(Command::SnapToGeometry), Some(&Chord::shift("K")));

    // The move never takes a chord from another command.
    let taken =
        stored.replace("\"snap_to_geometry\": \"V\"", "\"snap_to_geometry\": \"V\", \"toggle_bounding_box\": \"Ctrl\"");
    let map = Keymap::from_text(&taken).unwrap();
    assert_eq!(map.binding(Command::SnapToGeometry), Some(&Chord::key("V")));
    assert_eq!(map.binding(Command::ToggleBoundingBox), Some(&Chord::modifiers(true, false, false)));
    assert!(map.self_conflicts().is_empty());
}

#[test]
pub(crate) fn the_default_preset_pans_on_the_middle_button_alone() {
    // Issue 72: pan was behind a modifier, so the viewport seemed stuck around its target.
    let nav = Keymap::default().nav;
    assert_eq!(nav.pan, Drag::new(MouseButton::Middle), "pan is not the middle button on its own");
    assert!(!nav.pan.ctrl && !nav.pan.shift && !nav.pan.alt, "pan still asks for a modifier");
    assert!(
        !nav.orbit.matches(nav.pan.button, nav.pan.ctrl, nav.pan.shift, nav.pan.alt),
        "pan and orbit are the same drag"
    );
}

#[test]
pub(crate) fn a_moved_navigation_default_is_carried_over_but_a_chosen_drag_is_not() {
    // The same carry-over for `nav`'s old pan (issue 72).
    let stored = r#"{
      "preset": "default",
      "bindings": { "snap_to_geometry": "Ctrl" },
      "nav": { "orbit": { "button": "right" }, "pan": { "button": "right", "shift": true },
               "invert_zoom": false }
    }"#;
    let map = Keymap::from_text(stored).unwrap();
    assert_eq!(map.nav.pan, Drag::new(MouseButton::Middle), "the old default pan was not moved");
    assert_eq!(map.nav.orbit, Drag::new(MouseButton::Right), "moving pan disturbed orbit");

    // A pan the user chose is theirs.
    let chosen = stored.replace(r#""pan": { "button": "right", "shift": true }"#, r#""pan": { "button": "left" }"#);
    let map = Keymap::from_text(&chosen).unwrap();
    assert_eq!(map.nav.pan, Drag::new(MouseButton::Left), "a chosen pan was overwritten");

    // It never lands on the user's own orbit button.
    let taken = stored.replace(r#""orbit": { "button": "right" }"#, r#""orbit": { "button": "middle" }"#);
    let map = Keymap::from_text(&taken).unwrap();
    assert_eq!(map.nav.pan, Drag::with_shift(MouseButton::Right), "the move collided with orbit");

    // The other presets never had that default.
    for preset in [Preset::MeshEditor, Preset::Cad] {
        let text = Keymap::from_preset(preset).to_text();
        let back = Keymap::from_text(&text).unwrap();
        assert_eq!(back.nav, Keymap::from_preset(preset).nav, "{preset:?} lost its own navigation");
    }
}
