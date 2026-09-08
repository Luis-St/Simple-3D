//! A project written out and read back unchanged.

use super::*;
use crate::unit::Unit;

#[test]
pub(crate) fn a_project_round_trips_exactly() {
    // Spec acceptance criterion 17.
    let scene = sample();
    let text = to_string(&scene);
    let back = from_str(&text).expect("round trip");
    assert_eq!(fingerprint(&back), fingerprint(&scene));
}

#[test]
pub(crate) fn the_file_is_readable_and_diffable() {
    let text = to_string(&sample());
    assert!(text.starts_with("{\n"), "not pretty-printed");
    assert!(text.ends_with('\n'), "no trailing newline");
    assert!(text.contains(&format!("\"format\": {FORMAT_VERSION}")));
    assert!(text.contains("\"Drilled plate\""));
    // Every value on its own line, so a one-dimension change is a one-line diff.
    assert!(text.lines().count() > 30);
    // Saving twice produces the same bytes.
    assert_eq!(text, to_string(&sample()));
}

#[test]
pub(crate) fn a_stored_mesh_round_trips_through_the_file() {
    // Issue 80: a converted body is as much a part of the document as a
    // box, and a project that loses its geometry on save is worse than one
    // that never had it.
    use crate::mesh_data::MeshData;

    let mut scene = sample();
    let root = scene.root();
    let mesh = scene.add_mesh("Baked", MeshData::new(simple3d_geom::primitives::box_mesh(11.0, 12.0, 13.0)), root, 0);

    let text = to_string(&scene);
    let back = from_str(&text).expect("it should load again");
    let ids = back.depth_first();
    let name = scene.node(mesh).name.clone();
    let mesh_back = ids.iter().copied().find(|id| back.node(*id).name == name).expect("the mesh node");

    let stored = back.node(mesh_back).mesh().expect("the mesh body kept its geometry");
    assert_eq!(stored.triangle_count(), scene.node(mesh).mesh().unwrap().triangle_count());
    let (lo, hi) = stored.mesh.bounds().unwrap();
    assert!((hi.x - lo.x - 11.0).abs() < 1e-3, "the stored mesh came back {} wide", hi.x - lo.x);
}

#[test]
pub(crate) fn switching_the_display_unit_does_not_rescale_the_stored_model() {
    // Spec acceptance criterion 6, at the file level: the unit is metadata.
    let mut scene = sample();
    let before = to_string(&scene).replace("\"unit\": \"cm\"", "");
    scene.settings.unit = Unit::Metre;
    let after = to_string(&scene).replace("\"unit\": \"m\"", "");
    assert_eq!(before, after);
}
