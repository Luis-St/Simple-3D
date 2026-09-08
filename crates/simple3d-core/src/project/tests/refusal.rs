//! The files that are refused, and what the message says.

use super::*;
use crate::scene::Scene;

#[test]
pub(crate) fn a_newer_format_is_refused_with_a_clear_message() {
    let text = to_string(&sample()).replace(&format!("\"format\": {FORMAT_VERSION}"), "\"format\": 99");
    let err = from_str(&text).unwrap_err();
    assert_eq!(err, LoadError::TooNew { found: 99, supported: FORMAT_VERSION });
    assert!(err.to_string().contains("99"));
    assert!(err.to_string().contains("newer version of Simple 3D"));
}

#[test]
pub(crate) fn a_truncated_file_reports_where_it_stopped() {
    // Spec acceptance criterion 18.
    let text = to_string(&sample());
    let truncated = &text[..text.len() / 2];
    let err = from_str(truncated).unwrap_err();
    assert!(matches!(err, LoadError::Malformed(_)), "{err:?}");
    let message = err.to_string();
    assert!(message.contains("line"), "{message}");
}

#[test]
pub(crate) fn an_unknown_primitive_type_is_named() {
    let text = to_string(&sample()).replace("\"type\": \"plate\"", "\"type\": \"hyperboloid\"");
    let err = from_str(&text).unwrap_err();
    assert!(err.to_string().contains("hyperboloid"), "{err}");
}

#[test]
pub(crate) fn a_file_that_is_not_a_project_is_rejected() {
    for text in ["", "   ", "null", "[]", "{}", "{\"hello\": 1}", "not json at all"] {
        let err = from_str(text).unwrap_err();
        assert!(!err.to_string().is_empty(), "{text:?} produced an empty message");
    }
    // And never a silently empty scene.
    assert!(from_str("{}").is_err());
}

#[test]
pub(crate) fn a_mesh_body_whose_geometry_is_damaged_fails_the_file_rather_than_loading_empty() {
    use crate::mesh_data::MeshData;

    let mut scene = Scene::new();
    let root = scene.root();
    scene.add_mesh("Baked", MeshData::new(simple3d_geom::primitives::box_mesh(10.0, 10.0, 10.0)), root, 0);
    let text = to_string(&scene);
    // Cut the vertex array short, as a truncated copy or a bad edit would.
    let damaged = text.replacen("\"positions\": \"", "\"positions\": \"AAAA", 1);
    let err = from_str(&damaged).unwrap_err();
    assert!(err.to_string().contains("geometry"), "{err}");
}
