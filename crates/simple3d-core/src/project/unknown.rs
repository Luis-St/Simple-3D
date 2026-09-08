//! Primitive types a file names that this build does not have, so an older
//! build refuses a newer file instead of quietly dropping shapes.

use crate::scene::NodeData;

/// Names the primitive types the registry does not know, which is the only way
/// `replace_root` can fail.
pub(crate) fn unknown_types(root: &NodeData) -> String {
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

/// The node types that are not entries in the primitive registry. Each is a
/// [`crate::scene::Body`] of its own, so `lookup` will never find one and a
/// file holding one must not be reported as carrying an unknown shape.
pub(crate) const BODY_TYPES: &[&str] = &["group", "pattern", "mesh", "split"];

pub(crate) fn collect_unknown(node: &NodeData, out: &mut Vec<String>) {
    if !BODY_TYPES.contains(&node.type_id.as_str()) && crate::primitive::lookup(&node.type_id).is_none() {
        out.push(node.type_id.clone());
    }
    // A mesh node whose geometry cannot be read fails the load the same way an
    // unknown type does, and for the same reason: the alternative is a body
    // silently missing from whatever gets printed.
    if node.type_id == "mesh" && node.mesh.as_ref().and_then(crate::mesh_data::MeshData::from_blob).is_none() {
        out.push("mesh (its geometry could not be read)".to_string());
    }
    // A split carries the object it was broken from, which is a node like any
    // other: an unknown type in *there* fails the load too, and naming it is
    // what tells the user which shape the file is asking for.
    match &node.original {
        Some(original) => collect_unknown(original, out),
        None if node.type_id == "split" => out.push("split (the object it was made from is missing)".to_string()),
        None => {}
    }
    for child in &node.children {
        collect_unknown(child, out);
    }
}
