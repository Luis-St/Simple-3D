//! A project to text and back.

use super::*;
use crate::scene::Scene;
use serde::Deserialize;

pub fn to_string(scene: &Scene) -> String {
    let file = ProjectFile {
        format: FORMAT_VERSION,
        generator: format!("Simple 3D {}", env!("CARGO_PKG_VERSION")),
        settings: scene.settings.clone(),
        camera: scene.camera,
        root: scene.export_subtree(scene.root()).expect("the root always exports"),
    };
    // `to_string_pretty` plus a trailing newline: one value per line is what
    // makes a project file diffable.
    let mut text = serde_json::to_string_pretty(&file).expect("a scene always serialises");
    text.push('\n');
    text
}

pub fn from_str(text: &str) -> Result<Scene, LoadError> {
    // Read the version before anything else, so a file from a newer build gets
    // the version message rather than a confusing field error.
    #[derive(Deserialize)]
    struct VersionProbe {
        format: u32,
    }
    match serde_json::from_str::<VersionProbe>(text) {
        Ok(probe) if probe.format > FORMAT_VERSION => {
            return Err(LoadError::TooNew { found: probe.format, supported: FORMAT_VERSION })
        }
        Ok(_) => {}
        Err(e) if e.is_syntax() || e.is_eof() => return Err(LoadError::Malformed(describe(&e))),
        // Missing or wrongly-typed `format` field: not a project file.
        Err(e) => return Err(LoadError::Invalid(format!("no readable format version ({})", describe(&e)))),
    }

    let file: ProjectFile = serde_json::from_str(text).map_err(|e| {
        if e.is_syntax() || e.is_eof() {
            LoadError::Malformed(describe(&e))
        } else {
            LoadError::Invalid(describe(&e))
        }
    })?;

    let mut scene = Scene::new();
    scene.settings = file.settings;
    scene.camera = file.camera;
    if scene.replace_root(&file.root).is_none() {
        return Err(LoadError::Invalid(unknown_types(&file.root)));
    }
    // Never silently produce an empty scene from a file that had content.
    if !file.root.children.is_empty() && scene.node(scene.root()).children.is_empty() {
        return Err(LoadError::Invalid("the scene tree could not be reconstructed".into()));
    }
    Ok(scene)
}
