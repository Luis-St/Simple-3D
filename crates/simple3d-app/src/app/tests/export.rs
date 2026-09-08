//! What an export writes, and what it leaves out.

use super::*;
use simple3d_core::scene::{GroupOp, NodeId};
use simple3d_geom::Vec3;

/// Exporting a selected boolean group writes the shape the viewport shows,
/// not its operands. `Evaluated::node_meshes` holds primitives only, so
/// merging those wrote the cutter of a difference back into the result.
#[test]
pub(crate) fn exporting_a_selected_difference_writes_the_cut_shape_not_its_operands() {
    let mut app = app_in(temp_config_dir("export-selection"));
    let root = app.scene.root();
    let group = app.scene.add_group(GroupOp::Difference, root, 0);
    let plate = app.scene.add_primitive("plate", group, 0).expect("the plate is in the registry");
    let cutter = app.scene.add_primitive("box", group, 1).expect("the box is in the registry");
    // A cutter that pokes out through the top and the bottom, so a merged
    // export shows up as a taller box than the cut result can be.
    if let Some(node) = app.scene.get_mut(cutter) {
        node.position = Vec3::new(0.0, 0.0, 0.0);
    }
    app.reevaluate_for_test();
    let (plate_lo, plate_hi) = app.evaluated.node_meshes[&plate].bounds().expect("the plate has bounds");
    let (cut_lo, cut_hi) = app.evaluated.node_meshes[&cutter].bounds().expect("the cutter has bounds");
    assert!(cut_lo.z < plate_lo.z && cut_hi.z > plate_hi.z, "this test needs a cutter taller than the plate");

    app.selection = vec![group];
    app.export_selection_only = true;
    let mesh = app.export_mesh();
    let (lo, hi) = mesh.bounds().expect("the selection exported nothing");
    assert!(
        lo.z >= plate_lo.z - 1e-6 && hi.z <= plate_hi.z + 1e-6,
        "the export reaches {lo:?}..{hi:?}, beyond the plate it cut -- the cutter was written out too"
    );
    assert!(mesh.manifold_issue().is_none(), "the exported selection is not a closed solid");

    // The whole scene exports as it always did.
    app.export_selection_only = false;
    let (whole_lo, whole_hi) = app.export_mesh().bounds().expect("the scene exported nothing");
    assert!((whole_lo.z - lo.z).abs() < 1e-6 && (whole_hi.z - hi.z).abs() < 1e-6);
}

/// The selection outline draws the shape a node evaluates to, and stops
/// there. It used to expand to the node *and all its descendants*, so a
/// group's operands were outlined alongside its result: a difference's
/// cutter as two rims hanging in mid-air beside the solid, an
/// intersection's whole uncut box as a cage around the small lens it
/// leaves, a pattern's source child standing where no copy of it does.
#[test]
pub(crate) fn a_selected_group_is_outlined_as_its_result_not_as_its_operands() {
    let mut app = app_in(temp_config_dir("outline-group"));
    let root = app.scene.root();
    let group = app.scene.add_group(GroupOp::Difference, root, 0);
    let plate = app.scene.add_primitive("plate", group, 0).expect("the plate is in the registry");
    let cutter = app.scene.add_primitive("box", group, 1).expect("the box is in the registry");
    app.reevaluate_for_test();
    let (plate_lo, plate_hi) = app.evaluated.node_meshes[&plate].bounds().expect("the plate has bounds");
    let (cut_lo, cut_hi) = app.evaluated.node_meshes[&cutter].bounds().expect("the cutter has bounds");
    assert!(cut_lo.z < plate_lo.z && cut_hi.z > plate_hi.z, "this test needs a cutter taller than the plate");

    app.select_only(group);
    app.renderable_key = u64::MAX;
    app.refresh_node_renderables();
    let outlined: Vec<NodeId> = app.node_renderables.keys().copied().collect();
    assert_eq!(outlined, vec![group], "the operands were outlined too");

    let (lo, hi) = app.node_renderables[&group].mesh.bounds().expect("the group outlines nothing");
    assert!(
        lo.z >= plate_lo.z - 1e-6 && hi.z <= plate_hi.z + 1e-6,
        "the outline reaches {lo:?}..{hi:?}, past the plate the cut left -- it is drawing the cutter"
    );
}

/// Issue 58: a 3MF used to hold the whole scene as one component, so a
/// slicer had nothing to pick apart. The objects an export separates are
/// the scene's top-level nodes, each named and each the body it evaluates
/// to -- a difference is its cut shape, not its two operands.
#[test]
pub(crate) fn separating_objects_writes_one_per_top_level_node_by_name() {
    let mut app = app_in(temp_config_dir("export-parts"));
    let root = app.scene.root();
    let plate = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
    let group = app.scene.add_group(GroupOp::Difference, root, 1);
    let base = app.scene.add_primitive("plate", group, 0).expect("the plate is in the registry");
    let cutter = app.scene.add_primitive("box", group, 1).expect("the box is in the registry");
    if let Some(node) = app.scene.get_mut(plate) {
        node.name = "Lid".into();
        node.position = Vec3::new(200.0, 0.0, 0.0);
    }
    if let Some(node) = app.scene.get_mut(group) {
        node.name = "Drilled base".into();
    }
    app.reevaluate_for_test();
    app.export_bodies = simple3d_export::BodyMode::TopLevel;

    let parts = app.export_parts();
    let names: Vec<&str> = parts.iter().map(|part| part.name.as_str()).collect();
    assert_eq!(names, vec!["Lid", "Drilled base"], "one object per top-level node, in the outliner's order");

    // The group is its cut shape: the cutter pokes out of the plate, so a
    // part holding the operands would be taller than the plate ever is.
    let (plate_lo, plate_hi) = app.evaluated.node_meshes[&base].bounds().expect("the plate has bounds");
    let (cut_lo, cut_hi) = app.evaluated.node_meshes[&cutter].bounds().expect("the cutter has bounds");
    assert!(cut_lo.z < plate_lo.z && cut_hi.z > plate_hi.z, "this test needs a cutter taller than the plate");
    let (lo, hi) = parts[1].mesh.bounds().expect("the group exported nothing");
    assert!(
        lo.z >= plate_lo.z - 1e-6 && hi.z <= plate_hi.z + 1e-6,
        "the group's part reaches {lo:?}..{hi:?}, beyond the plate it cut -- its operands were written out"
    );

    // Each is a closed solid on its own, which is what letting the exporter
    // verify them separately depends on.
    for part in &parts {
        assert!(part.mesh.manifold_issue().is_none(), "{} is not a closed solid", part.name);
    }

    // And the mode only takes effect where the format can hold it.
    app.export_bodies = simple3d_export::BodyMode::TopLevel;
    app.export_format = simple3d_export::Format::ThreeMf;
    assert_eq!(app.export_body_mode(), simple3d_export::BodyMode::TopLevel);
    app.export_format = simple3d_export::Format::StlBinary;
    assert_eq!(
        app.export_body_mode(),
        simple3d_export::BodyMode::One,
        "STL holds one body; separating them there is not a thing to promise"
    );
}
