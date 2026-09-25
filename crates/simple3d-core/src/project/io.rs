//! A project to text and back.

use super::*;
use crate::scene::Scene;
use serde::Deserialize;

/// A project of one component, the way every project was before there could
/// be more.
pub fn to_string(scene: &Scene) -> String {
    write(scene, &[])
}

/// A whole project, with its root component first.
pub fn project_to_string(root: &Scene, components: &[(ComponentId, &Scene)]) -> String {
    write(root, components)
}

fn write(scene: &Scene, components: &[(ComponentId, &Scene)]) -> String {
    let file = ProjectFile {
        format: if components.is_empty() { PLAIN_FORMAT } else { FORMAT_VERSION },
        generator: format!("Simple 3D {}", env!("CARGO_PKG_VERSION")),
        settings: scene.settings.clone(),
        camera: scene.camera,
        root: scene.export_subtree(scene.root()).expect("the root always exports"),
        components: components
            .iter()
            .map(|(id, scene)| ComponentFile {
                id: *id,
                settings: scene.settings.clone(),
                camera: scene.camera,
                root: scene.export_subtree(scene.root()).expect("the root always exports"),
            })
            .collect(),
    };
    // `to_string_pretty` plus a trailing newline: one value per line is what
    // makes a project file diffable.
    let mut text = serde_json::to_string_pretty(&file).expect("a scene always serialises");
    text.push('\n');
    text
}

/// The root component of a project, for a caller that only wants that.
pub fn from_str(text: &str) -> Result<Scene, LoadError> {
    project_from_str(text).map(|project| project.root)
}

/// A whole project. A file written before components existed is a project of
/// its root component alone, which is what it always was.
pub fn project_from_str(text: &str) -> Result<ProjectData, LoadError> {
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

    let root = read_scene(file.settings, file.camera, &file.root)?;
    let mut components: Vec<(ComponentId, Scene)> = Vec::new();
    for component in file.components {
        // Two components under one id could not both be reached, and the root
        // component's id is the root's alone.
        if component.id == crate::scene::ROOT_COMPONENT || components.iter().any(|(id, _)| *id == component.id) {
            return Err(LoadError::Invalid(format!("component {} appears more than once", component.id)));
        }
        let scene = read_scene(component.settings, component.camera, &component.root)?;
        components.push((component.id, scene));
    }
    Ok(ProjectData { root, components })
}

fn read_scene(settings: SceneSettings, camera: Camera, root: &NodeData) -> Result<Scene, LoadError> {
    let mut scene = Scene::new();
    scene.settings = settings;
    scene.camera = camera;
    if scene.replace_root(root).is_none() {
        return Err(LoadError::Invalid(unknown_types(root)));
    }
    // Never silently produce an empty scene from a file that had content.
    if !root.children.is_empty() && scene.node(scene.root()).children.is_empty() {
        return Err(LoadError::Invalid("the scene tree could not be reconstructed".into()));
    }
    Ok(scene)
}
