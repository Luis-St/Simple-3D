//! The optional parts of a document: colours, section and export bodies.

use super::*;

#[test]
pub(crate) fn a_section_plane_is_saved_with_the_document_and_absent_while_it_is_off() {
    // It is a view of the model, not a change to it, but the offset is a
    // place *in this model*: it belongs to the document the way the camera
    // does. Off, it writes nothing at all, so a project made by this build
    // still diffs cleanly against one made before sections existed.
    let mut scene = sample();
    assert!(!to_string(&scene).contains("section"), "an unused section was written to the file");

    scene.settings.section = crate::scene::SectionView { enabled: true, axis: 1, offset: 12.5, flipped: true };
    let text = to_string(&scene);
    let back = from_str(&text).expect("it reads back");
    assert_eq!(back.settings.section, scene.settings.section);

    // And a file written before the field existed still opens, with the
    // section simply off.
    let older = text.replace("\"section\":", "\"unknown_to_this_build\":");
    assert!(!from_str(&older).expect("an older file still opens").settings.section.enabled);
}

#[test]
pub(crate) fn a_colour_survives_the_file_and_reads_as_hex() {
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
pub(crate) fn an_unpainted_scene_writes_no_colour_at_all() {
    // A file written by this version has to diff cleanly against one
    // written before colours existed.
    assert!(!to_string(&sample()).contains("colour"));
}

#[test]
pub(crate) fn export_bodies_survive_the_file_so_a_re_export_only_needs_what_changed() {
    // Issue 58: the whole point of choosing the bodies is not having to
    // choose them again. They live on the nodes, so saving carries them.
    use crate::scene::{ExportBody, GroupOp};

    let mut scene = sample();
    let root = scene.root();
    let first = scene.node(root).children[0];
    let group = scene.add_group(GroupOp::Union, root, 1);
    let inner = scene.add_primitive("box", group, 0).expect("the box is in the registry");
    scene.set_export_body(first, Some(ExportBody::Shared(2)));
    scene.set_export_body(group, Some(ExportBody::Split));
    scene.set_export_body(inner, Some(ExportBody::Shared(2)));

    let text = to_string(&scene);
    let back = from_str(&text).expect("it should load again");
    let first = back.node(back.root()).children[0];
    let group = back.node(back.root()).children[1];
    let inner = back.node(group).children[0];
    assert_eq!(back.node(first).export_body, Some(ExportBody::Shared(2)));
    assert_eq!(back.node(group).export_body, Some(ExportBody::Split));
    assert_eq!(back.node(inner).export_body, Some(ExportBody::Shared(2)), "the mark inside the group was lost");
}

#[test]
pub(crate) fn a_colour_that_is_not_a_colour_loads_as_unpainted() {
    // A hand-edited or truncated value must not fail the whole file.
    let text = to_string(&sample()).replace("\"visible\": true", "\"visible\": true, \"colour\": \"nonsense\"");
    let scene = from_str(&text).expect("the file should still load");
    assert!(scene.node(scene.node(scene.root()).children[0]).colour.is_none());
}
