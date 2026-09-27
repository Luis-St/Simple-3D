//! Primitive types a file names that this build lacks, so an older build refuses a newer file
//! instead of dropping shapes.

use crate::scene::NodeData;

/// Names the unknown primitive types; the only way `replace_root` can fail.
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

/// Node types that are bodies of their own, not registry entries, so not unknown shapes.
pub(crate) const BODY_TYPES: &[&str] = &["group", "pattern", "mesh", "split", "component"];

pub(crate) fn collect_unknown(node: &NodeData, out: &mut Vec<String>) {
    if !BODY_TYPES.contains(&node.type_id.as_str()) && crate::primitive::lookup(&node.type_id).is_none() {
        out.push(node.type_id.clone());
    }
    // An unreadable mesh fails the load too, rather than a body silently missing from the print.
    if node.type_id == "mesh" && node.mesh.as_ref().and_then(crate::mesh_data::MeshData::from_blob).is_none() {
        out.push("mesh (its geometry could not be read)".to_string());
    }
    if node.type_id == "component" && node.component.is_none() {
        out.push("component (it does not say which one)".to_string());
    }
    // A split's original is checked as well, so the error names the shape the file asks for.
    match &node.original {
        Some(original) => collect_unknown(original, out),
        None if node.type_id == "split" => out.push("split (the object it was made from is missing)".to_string()),
        None => {}
    }
    for child in &node.children {
        collect_unknown(child, out);
    }
}
