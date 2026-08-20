//! The project file (spec section 10).
//!
//! One user-visible file holding the entire scene, the display unit, the scene
//! settings and the camera. Human-readable and text-based -- pretty-printed JSON
//! with `BTreeMap`-ordered keys -- so projects diff cleanly and can go under
//! version control. Node subtrees use the same `NodeData` schema as the
//! clipboard, so a selection copied to the clipboard can be pasted into a text
//! editor and back again.

use crate::scene::{Camera, NodeData, Scene, SceneSettings};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Bumped whenever the schema changes in a way an older build could not read.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    format: u32,
    #[serde(default)]
    generator: String,
    settings: SceneSettings,
    camera: Camera,
    root: NodeData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// A file written by a newer version. Refused with a clear message rather
    /// than half-understood.
    TooNew { found: u32, supported: u32 },
    /// Not valid JSON at all -- truncated mid-file, or not a project file. The
    /// message names the position and what was expected.
    Malformed(String),
    /// Valid JSON, but not a valid scene: an unknown primitive type, a missing
    /// required field.
    Invalid(String),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::TooNew { found, supported } => write!(
                f,
                "This project was saved in format version {found}, but this build only understands \
                 version {supported}. Use a newer version of Simple 3D to open it."
            ),
            LoadError::Malformed(why) => write!(f, "The project file could not be read: {why}"),
            LoadError::Invalid(why) => write!(f, "The project file is not a valid scene: {why}"),
        }
    }
}

impl std::error::Error for LoadError {}

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

fn describe(e: &serde_json::Error) -> String {
    format!("{e}")
}

/// Names the primitive types the registry does not know, which is the only way
/// `replace_root` can fail.
fn unknown_types(root: &NodeData) -> String {
    let mut unknown: Vec<String> = Vec::new();
    collect_unknown(root, &mut unknown);
    unknown.sort();
    unknown.dedup();
    match unknown.len() {
        0 => "the scene tree could not be reconstructed".into(),
        1 => format!("unknown primitive type \"{}\"", unknown[0]),
        _ => format!("unknown primitive types: {}", unknown.join(", ")),
    }
}

fn collect_unknown(node: &NodeData, out: &mut Vec<String>) {
    if node.type_id != "group" && crate::primitive::lookup(&node.type_id).is_none() {
        out.push(node.type_id.clone());
    }
    for child in &node.children {
        collect_unknown(child, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitive::ParamValue;
    use crate::scene::{Anchor, GroupOp};
    use crate::unit::Unit;
    use simple3d_geom::Vec3;

    fn sample() -> Scene {
        let mut scene = Scene::new();
        scene.settings.unit = Unit::Centimetre;
        scene.settings.default_segments = 48;
        scene.settings.notes = "A bracket".into();
        scene.settings.grid_spacing = 5.0;
        scene.camera.distance = 321.5;
        scene.camera.yaw = -12.5;
        scene.camera.pitch = 41.25;
        let root = scene.root();
        let group = scene.add_group(GroupOp::Difference, root, 0);
        scene.get_mut(group).unwrap().name = "Drilled plate".into();
        let plate = scene.add_primitive("plate", group, 0).unwrap();
        scene.get_mut(plate).unwrap().anchor = Anchor::Base;
        let hole = scene.add_primitive("cylinder", group, 1).unwrap();
        scene.get_mut(hole).unwrap().position = Vec3::new(-8.0, 0.0, 0.0);
        scene.get_mut(hole).unwrap().rotation = Vec3::new(0.0, 0.0, 30.0);
        scene.get_mut(hole).unwrap().segments = Some(64);
        scene.get_mut(hole).unwrap().visible = false;
        scene.get_mut(plate).unwrap().scale = Vec3::new(1.5, 1.0, 0.25);
        scene.get_mut(hole).unwrap().params_mut().unwrap().insert("diameter_x".into(), ParamValue::Length(6.0));
        scene
    }

    fn fingerprint(scene: &Scene) -> String {
        let mut out = format!("{:?}{:?}", scene.settings, scene.camera);
        for id in scene.depth_first() {
            let node = scene.node(id);
            out.push_str(&format!(
                "{}|{}|{:?}|{:?}|{:?}|{:?}|{}|{:?}|{:?};",
                scene.depth(id),
                node.name,
                node.position,
                node.rotation,
                node.scale,
                node.anchor,
                node.visible,
                node.segments,
                node.params(),
            ));
        }
        out
    }

    #[test]
    fn a_project_round_trips_exactly() {
        // Spec acceptance criterion 17.
        let scene = sample();
        let text = to_string(&scene);
        let back = from_str(&text).expect("round trip");
        assert_eq!(fingerprint(&back), fingerprint(&scene));
    }

    #[test]
    fn the_file_is_readable_and_diffable() {
        let text = to_string(&sample());
        assert!(text.starts_with("{\n"), "not pretty-printed");
        assert!(text.ends_with('\n'), "no trailing newline");
        assert!(text.contains("\"format\": 1"));
        assert!(text.contains("\"Drilled plate\""));
        // Every value on its own line, so a one-dimension change is a one-line diff.
        assert!(text.lines().count() > 30);
        // Saving twice produces the same bytes.
        assert_eq!(text, to_string(&sample()));
    }

    #[test]
    fn a_newer_format_is_refused_with_a_clear_message() {
        let text = to_string(&sample()).replace("\"format\": 1", "\"format\": 99");
        let err = from_str(&text).unwrap_err();
        assert_eq!(err, LoadError::TooNew { found: 99, supported: FORMAT_VERSION });
        assert!(err.to_string().contains("99"));
        assert!(err.to_string().contains("newer version of Simple 3D"));
    }

    #[test]
    fn a_truncated_file_reports_where_it_stopped() {
        // Spec acceptance criterion 18.
        let text = to_string(&sample());
        let truncated = &text[..text.len() / 2];
        let err = from_str(truncated).unwrap_err();
        assert!(matches!(err, LoadError::Malformed(_)), "{err:?}");
        let message = err.to_string();
        assert!(message.contains("line"), "{message}");
    }

    #[test]
    fn an_unknown_primitive_type_is_named() {
        let text = to_string(&sample()).replace("\"type\": \"plate\"", "\"type\": \"hyperboloid\"");
        let err = from_str(&text).unwrap_err();
        assert!(err.to_string().contains("hyperboloid"), "{err}");
    }

    #[test]
    fn a_file_that_is_not_a_project_is_rejected() {
        for text in ["", "   ", "null", "[]", "{}", "{\"hello\": 1}", "not json at all"] {
            let err = from_str(text).unwrap_err();
            assert!(!err.to_string().is_empty(), "{text:?} produced an empty message");
        }
        // And never a silently empty scene.
        assert!(from_str("{}").is_err());
    }

    #[test]
    fn an_older_file_missing_optional_fields_migrates_silently() {
        // Spec section 10: "loading an older one migrates silently where possible".
        let minimal = r#"{
          "format": 1,
          "settings": { "unit": "mm", "default_segments": 32, "grid_spacing": 10.0, "grid_visible": true },
          "camera": { "target": {"x":0,"y":0,"z":0}, "distance": 160.0, "yaw": 0.0, "pitch": 20.0,
                      "orthographic": false, "fov_deg": 45.0 },
          "root": { "name": "Scene", "type": "group", "op": "union",
                    "children": [ { "name": "Plate", "type": "plate" } ] }
        }"#;
        let scene = from_str(minimal).expect("minimal file should load");
        let child = scene.node(scene.root()).children[0];
        assert_eq!(scene.node(child).name, "Plate");
        // Defaults filled in: visible, centre anchor, and every declared parameter.
        assert!(scene.node(child).visible);
        assert_eq!(scene.node(child).anchor, Anchor::Centre);
        let plate = crate::primitive::lookup("plate").expect("the plate is a declared type");
        assert_eq!(scene.node(child).params().unwrap().len(), plate.params.len());
        assert_eq!(scene.settings.notes, "");
    }

    #[test]
    fn a_colour_survives_the_file_and_reads_as_hex() {
        let mut scene = sample();
        let child = scene.node(scene.root()).children[0];
        scene.paint_subtree(child, Some(crate::scene::Colour([0x2E, 0x9A, 0xFF])));
        let text = to_string(&scene);
        assert!(text.contains("\"#2e9aff\""), "the colour should be readable in the file: {text}");
        let back = from_str(&text).expect("it should load again");
        let child = back.node(back.root()).children[0];
        assert_eq!(back.node(child).colour.map(|c| c.0), Some([0x2E, 0x9A, 0xFF]));
    }

    #[test]
    fn an_unpainted_scene_writes_no_colour_at_all() {
        // A file written by this version has to diff cleanly against one
        // written before colours existed.
        assert!(!to_string(&sample()).contains("colour"));
    }

    #[test]
    fn a_colour_that_is_not_a_colour_loads_as_unpainted() {
        // A hand-edited or truncated value must not fail the whole file.
        let text = to_string(&sample()).replace("\"visible\": true", "\"visible\": true, \"colour\": \"nonsense\"");
        let scene = from_str(&text).expect("the file should still load");
        assert!(scene.node(scene.node(scene.root()).children[0]).colour.is_none());
    }

    #[test]
    fn switching_the_display_unit_does_not_rescale_the_stored_model() {
        // Spec acceptance criterion 6, at the file level: the unit is metadata.
        let mut scene = sample();
        let before = to_string(&scene).replace("\"unit\": \"cm\"", "");
        scene.settings.unit = Unit::Metre;
        let after = to_string(&scene).replace("\"unit\": \"m\"", "");
        assert_eq!(before, after);
    }
}
