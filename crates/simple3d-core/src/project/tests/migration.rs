//! Older files, and the fields they do not carry.

use super::*;
use crate::scene::Anchor;

#[test]
pub(crate) fn an_older_file_missing_optional_fields_migrates_silently() {
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
pub(crate) fn a_scene_nobody_has_grouped_writes_no_export_body_at_all() {
    // A file written by this version has to diff cleanly against one
    // written before export bodies existed.
    assert!(!to_string(&sample()).contains("export_body"));
}
