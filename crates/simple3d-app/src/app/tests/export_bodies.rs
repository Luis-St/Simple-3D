//! Which nodes become objects of their own in the written file.

use super::*;
use simple3d_core::scene::GroupOp;
use simple3d_geom::Vec3;

/// The user-chosen export mode: marks live on nodes and unmarked nodes are bodies of their own, so
/// an unmarked project exports like "top level bodies".
#[test]
pub(crate) fn user_selected_bodies_merge_what_is_marked_and_split_what_is_opened() {
    use simple3d_core::scene::ExportBody;

    let mut app = app_in(temp_config_dir("export-bodies"));
    let root = app.scene.root();
    let plate = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
    let bracket = app.scene.add_primitive("box", root, 1).expect("the box is in the registry");
    let frame = app.scene.add_group(GroupOp::Union, root, 2);
    let left = app.scene.add_primitive("box", frame, 0).expect("the box is in the registry");
    let right = app.scene.add_primitive("box", frame, 1).expect("the box is in the registry");
    for (id, name, x) in
        [(plate, "Plate", 0.0), (bracket, "Bracket", 200.0), (left, "Left rail", 0.0), (right, "Right rail", 60.0)]
    {
        let node = app.scene.get_mut(id).unwrap();
        node.name = name.into();
        node.position = Vec3::new(x, 0.0, 0.0);
    }
    app.scene.get_mut(frame).unwrap().name = "Frame".into();
    app.scene.get_mut(frame).unwrap().position = Vec3::new(0.0, 300.0, 0.0);
    app.reevaluate_for_test();
    app.export_bodies = simple3d_export::BodyMode::Selected;

    // Untouched, it matches the top-level answer.
    let names: Vec<String> = app.export_parts().into_iter().map(|part| part.name).collect();
    assert_eq!(names, vec!["Plate", "Bracket", "Frame"], "an unmarked project is not the top-level grouping");

    // The same number on two shapes makes one body named for both.
    app.scene.set_export_body(plate, Some(ExportBody::Shared(1)));
    app.scene.set_export_body(bracket, Some(ExportBody::Shared(1)));
    let parts = app.export_parts();
    let names: Vec<&str> = parts.iter().map(|part| part.name.as_str()).collect();
    assert_eq!(names, vec!["Plate + Bracket", "Frame"], "the two marked shapes did not become one body");
    // Merged, not appended: one closed surface, or the export refuses it.
    assert!(parts[0].mesh.manifold_issue().is_none(), "the merged body is not a closed solid");

    // Splitting a group offers its contents in its place.
    app.scene.set_export_body(frame, Some(ExportBody::Split));
    let names: Vec<String> = app.export_parts().into_iter().map(|part| part.name).collect();
    assert_eq!(names, vec!["Plate + Bracket", "Left rail", "Right rail"], "splitting the group did not reach in");

    // A body reaches across the tree: a rail can join the plate.
    app.scene.set_export_body(left, Some(ExportBody::Shared(1)));
    let names: Vec<String> = app.export_parts().into_iter().map(|part| part.name).collect();
    assert_eq!(names, vec!["Plate + Bracket + Left rail", "Right rail"], "a body did not reach into the group");

    // Closing the group drops the marks inside it.
    app.scene.set_export_body(frame, None);
    assert_eq!(app.scene.node(left).export_body, None, "a mark survived the group being closed over it");
    let names: Vec<String> = app.export_parts().into_iter().map(|part| part.name).collect();
    assert_eq!(names, vec!["Plate + Bracket", "Frame"]);
}

/// A shape added later is the only new decision; existing marks stand.
#[test]
pub(crate) fn a_shape_added_later_is_the_only_body_left_to_place() {
    use simple3d_core::scene::ExportBody;

    let mut app = app_in(temp_config_dir("export-bodies-reexport"));
    let root = app.scene.root();
    let plate = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
    let bracket = app.scene.add_primitive("box", root, 1).expect("the box is in the registry");
    app.scene.get_mut(plate).unwrap().name = "Plate".into();
    app.scene.get_mut(bracket).unwrap().name = "Bracket".into();
    app.scene.get_mut(bracket).unwrap().position = Vec3::new(200.0, 0.0, 0.0);
    app.export_bodies = simple3d_export::BodyMode::Selected;
    app.scene.set_export_body(plate, Some(ExportBody::Shared(1)));
    app.scene.set_export_body(bracket, Some(ExportBody::Shared(1)));
    app.reevaluate_for_test();
    assert_eq!(app.export_parts().len(), 1, "the two marked shapes are one body");

    // A new shape; the earlier marks still stand.
    let lid = app.scene.add_primitive("box", root, 2).expect("the box is in the registry");
    app.scene.get_mut(lid).unwrap().name = "Lid".into();
    app.scene.get_mut(lid).unwrap().position = Vec3::new(400.0, 0.0, 0.0);
    app.reevaluate_for_test();
    let names: Vec<String> = app.export_parts().into_iter().map(|part| part.name).collect();
    assert_eq!(names, vec!["Plate + Bracket", "Lid"], "the grouping did not survive the scene changing");

    // Placing it is one choice.
    app.scene.set_export_body(lid, Some(ExportBody::Shared(1)));
    let names: Vec<String> = app.export_parts().into_iter().map(|part| part.name).collect();
    assert_eq!(names, vec!["Plate + Bracket + Lid"]);
}

/// End to end through the real writer: grouped bodies become the file's components.
#[test]
pub(crate) fn the_written_3mf_holds_one_named_component_per_chosen_body() {
    use simple3d_core::scene::ExportBody;

    let dir = temp_config_dir("export-bodies-file");
    let mut app = app_in(dir.clone());
    let root = app.scene.root();
    let plate = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
    let bracket = app.scene.add_primitive("box", root, 1).expect("the box is in the registry");
    let lid = app.scene.add_primitive("box", root, 2).expect("the box is in the registry");
    for (id, name, x) in [(plate, "Plate", 0.0), (bracket, "Bracket", 200.0), (lid, "Lid", 400.0)] {
        let node = app.scene.get_mut(id).unwrap();
        node.name = name.into();
        node.position = Vec3::new(x, 0.0, 0.0);
    }
    app.reevaluate_for_test();
    app.export_bodies = simple3d_export::BodyMode::Selected;
    app.scene.set_export_body(plate, Some(ExportBody::Shared(1)));
    app.scene.set_export_body(bracket, Some(ExportBody::Shared(1)));

    // Exactly what `start_export` hands the job, written by the job's own call.
    let parts = app.export_parts();
    let meshes: Vec<(String, simple3d_geom::Mesh)> = parts.into_iter().map(|p| (p.name, p.mesh)).collect();
    let borrowed: Vec<simple3d_export::Part<'_>> =
        meshes.iter().map(|(name, mesh)| simple3d_export::Part { name, mesh }).collect();
    let options = simple3d_export::Options {
        format: simple3d_export::Format::ThreeMf,
        bodies: app.export_body_mode(),
        ..Default::default()
    };
    let path = dir.join("bodies.3mf");
    simple3d_export::write_parts(&path, &borrowed, &options, &mut |_| true).expect("the export should succeed");

    // Read as a slicer would, since the package is compressed by default.
    let package = std::fs::read(&path).unwrap();
    let archive = simple3d_import::unzip::Archive::open(&package).expect("the export wrote a package");
    let text = String::from_utf8(
        archive.read_by(|name| name == "3D/3dmodel.model").expect("it holds a model part").expect("it is readable"),
    )
    .expect("the model document is text");
    assert_eq!(text.matches("<object id=").count(), 2, "{text}");
    assert!(text.contains("name=\"Plate + Bracket\""), "the merged body is not named for what is in it");
    assert!(text.contains("name=\"Lid\""), "the untouched shape lost its own body");
    assert_eq!(text.matches("<item objectid=").count(), 2, "both bodies have to be in the build");
}

/// A difference is one new surface; its operands are never written as bodies.
#[test]
pub(crate) fn a_boolean_that_fuses_its_operands_cannot_be_split_into_them() {
    use simple3d_core::scene::ExportBody;

    let mut app = app_in(temp_config_dir("export-bodies-fused"));
    let root = app.scene.root();
    let cut = app.scene.add_group(GroupOp::Difference, root, 0);
    let base = app.scene.add_primitive("plate", cut, 0).expect("the plate is in the registry");
    let bore = app.scene.add_primitive("box", cut, 1).expect("the box is in the registry");
    app.scene.get_mut(cut).unwrap().name = "Drilled".into();
    app.reevaluate_for_test();
    app.export_bodies = simple3d_export::BodyMode::Selected;

    assert!(!app.scene.can_split_for_export(cut), "a difference offered to be split into its operands");
    assert!(app.scene.can_split_for_export(app.scene.root()), "the scene's own top level is a union");

    // Even when marked directly (hand-edited file, or a union later made a difference).
    app.scene.get_mut(cut).unwrap().export_body = Some(ExportBody::Split);
    app.scene.get_mut(bore).unwrap().export_body = Some(ExportBody::Shared(2));
    let parts = app.export_parts();
    let names: Vec<&str> = parts.iter().map(|part| part.name.as_str()).collect();
    assert_eq!(names, vec!["Drilled"], "the difference was taken apart into its operands");

    let (plate_lo, plate_hi) = app.evaluated.node_meshes[&base].bounds().expect("the plate has bounds");
    let (lo, hi) = parts[0].mesh.bounds().expect("the difference exported nothing");
    assert!(
        lo.z >= plate_lo.z - 1e-6 && hi.z <= plate_hi.z + 1e-6,
        "the exported body reaches {lo:?}..{hi:?}, past the plate it cut"
    );
}
