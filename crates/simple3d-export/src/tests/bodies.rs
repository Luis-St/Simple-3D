//! Several objects in one file, or merged into one.

use super::*;
use simple3d_geom::primitives;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn separate_objects_write_one_component_each_with_its_node_s_name() {
    // Issue 58: a 3MF whose objects are all one component gives a slicer
    // nothing to select. Two boxes, well apart, exported separately.
    let left = plate();
    let right = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(100.0, 0.0, 0.0));
    let parts = [Part { name: "Base plate", mesh: &left }, Part { name: "Peg", mesh: &right }];
    let options = Options { bodies: BodyMode::TopLevel, ..Default::default() };

    let bytes = three_mf(&parts, &options, &mut no_progress()).unwrap();
    let text = String::from_utf8_lossy(&bytes).to_string();
    assert_eq!(text.matches("<object id=").count(), 2, "{text}");
    assert!(text.contains("<object id=\"1\" type=\"model\" name=\"Base plate\">"), "{text}");
    assert!(text.contains("<object id=\"2\" type=\"model\" name=\"Peg\">"), "{text}");
    // Both have to be in the build, or a slicer loads an empty plate.
    assert!(text.contains("<item objectid=\"1\"/>"), "{text}");
    assert!(text.contains("<item objectid=\"2\"/>"), "{text}");
    // Every triangle of both is there: the split is in the file's structure,
    // not in what it holds.
    assert_eq!(text.matches("<triangle ").count(), left.weld().indices.len() + right.weld().indices.len(), "{text}");
}

#[test]
pub(crate) fn the_same_objects_merge_into_one_component_when_the_option_is_off() {
    // The other half of the assertion above: without the option the file is
    // exactly the single-object one it always was.
    let left = plate();
    let right = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(100.0, 0.0, 0.0));
    let parts = [Part { name: "Base plate", mesh: &left }, Part { name: "Peg", mesh: &right }];
    let path = temp_dir().join("merged.3mf");
    write_parts(&path, &parts, &Options::default(), &mut no_progress()).unwrap();
    let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).to_string();
    assert_eq!(text.matches("<object id=").count(), 1, "{text}");
    assert_eq!(text.matches("<item objectid=").count(), 1, "{text}");
    assert!(!text.contains("name=\"Base plate\""), "a merged body has no part names to give");
    std::fs::remove_file(&path).unwrap();
}

#[test]
pub(crate) fn a_separated_object_is_verified_on_its_own_and_named_when_it_fails() {
    // Merged, a hole in one body can be hidden by the surface of another;
    // kept apart it cannot, and the message has to say which object to fix.
    let good = plate();
    let mut broken = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(100.0, 0.0, 0.0));
    broken.indices.pop();
    broken.tags.pop();
    let parts = [Part { name: "Base plate", mesh: &good }, Part { name: "Peg", mesh: &broken }];
    let path = temp_dir().join("broken-part.3mf");
    let options = Options { bodies: BodyMode::TopLevel, ..Default::default() };
    let err = write_parts(&path, &parts, &options, &mut no_progress()).unwrap_err();
    match err {
        ExportError::Invalid(problems) => {
            assert!(problems.iter().all(|p| p.starts_with("Peg: ")), "{problems:?}");
        }
        other => panic!("expected the broken part to be reported, got {other:?}"),
    }
    assert!(!path.exists(), "a refused export left a file behind");
}

#[test]
pub(crate) fn a_name_with_xml_in_it_cannot_break_the_document() {
    let mesh = plate();
    let other = primitives::box_mesh(10.0, 10.0, 10.0).translated(Vec3::new(100.0, 0.0, 0.0));
    let parts = [Part { name: "<Bracket & \"clip\">", mesh: &mesh }, Part { name: "B", mesh: &other }];
    let options = Options { bodies: BodyMode::TopLevel, ..Default::default() };
    let text = String::from_utf8_lossy(&three_mf(&parts, &options, &mut no_progress()).unwrap()).to_string();
    assert!(text.contains("name=\"&lt;Bracket &amp; &quot;clip&quot;&gt;\""), "{text}");
}
