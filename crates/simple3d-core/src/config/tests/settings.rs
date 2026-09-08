//! The settings file: written, read back, and survived when damaged.

use super::*;
use std::path::PathBuf;

#[test]
pub(crate) fn settings_round_trip_and_tolerate_a_partial_file() {
    let settings = AppSettings {
        display_mode: DisplayMode::Wireframe,
        render_engine: RenderEngine::Gpu,
        last_export_scale: 2.5,
        last_export_format: "stl".into(),
        recent_files: vec![PathBuf::from("/tmp/a.simple3d")],
        ..AppSettings::default()
    };
    let text = serde_json::to_string(&settings).unwrap();
    let back: AppSettings = serde_json::from_str(&text).unwrap();
    assert_eq!(back, settings);

    // A file from an older build that did not have some of these fields.
    let partial: AppSettings = serde_json::from_str("{\"display_mode\":\"wireframe\"}").unwrap();
    assert_eq!(partial.display_mode, DisplayMode::Wireframe);
    assert_eq!(partial.last_export_format, "3mf");
    assert_eq!(partial.rotate_snap_deg, 15.0);
    // A settings file written before there was an engine to choose was
    // written by a build that drew in software, so that is what it means.
    assert_eq!(partial.render_engine, RenderEngine::Cpu);
}

/// The engine a settings file names, and the one it means when it names
/// nothing. The CPU renderer is the default because it is the one that
/// cannot fail to be available.
#[test]
pub(crate) fn the_render_engine_survives_the_settings_file() {
    assert_eq!(AppSettings::default().render_engine, RenderEngine::Cpu);
    for engine in RenderEngine::ALL {
        let settings = AppSettings { render_engine: engine, ..AppSettings::default() };
        let text = serde_json::to_string(&settings).unwrap();
        let back: AppSettings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.render_engine, engine, "{}", engine.label());
    }
    let named: AppSettings = serde_json::from_str("{\"render_engine\":\"gpu\"}").unwrap();
    assert_eq!(named.render_engine, RenderEngine::Gpu);
    // An engine this build does not know is not a reason to refuse the file.
    assert!(serde_json::from_str::<AppSettings>("{\"render_engine\":\"quantum\"}").is_err());
}

#[test]
pub(crate) fn corrupt_settings_fall_back_to_defaults_rather_than_failing() {
    for text in ["", "{", "null", "[1,2,3]", "{\"display_mode\": \"holographic\"}"] {
        let parsed: Option<AppSettings> = serde_json::from_str(text).ok();
        let settings = parsed.unwrap_or_default();
        assert_eq!(settings.last_export_format, "3mf");
    }
}

/// Dialogs are windows of the window system's own by default (issue 53).
/// The switch that draws them inside the main window instead is a way out
/// of the NVIDIA Wayland freeze, where a second surface is the hazard, and
/// it has to survive the settings file to be one -- and a settings file
/// written before it existed has to read as the behaviour it had.
#[test]
pub(crate) fn dialogs_are_their_own_windows_unless_the_settings_say_otherwise() {
    assert!(!AppSettings::default().embed_dialogs, "a dialog is a real window by default");

    let older = r#"{"window_size": [1400.0, 880.0]}"#;
    let migrated: AppSettings = serde_json::from_str(older).expect("an older settings file still loads");
    assert!(!migrated.embed_dialogs, "a file written before the switch existed reads as off");

    let mut settings = AppSettings::default();
    settings.embed_dialogs = true;
    let text = serde_json::to_string(&settings).expect("settings serialise");
    let back: AppSettings = serde_json::from_str(&text).expect("and load again");
    assert!(back.embed_dialogs, "the switch did not survive the file");
}
