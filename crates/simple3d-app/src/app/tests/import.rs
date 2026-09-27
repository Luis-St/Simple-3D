//! Importing a model file (issue 105): what it lands as, what it reports, and when nothing comes in.

use super::*;
use crate::worker::ImportJob;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

/// Write `mesh` in `format`, then import it the application's way: job thread plus `poll_import`.
fn import_written(app: &mut App, mesh: &simple3d_geom::Mesh, format: simple3d_export::Format) {
    let path = temp_config_dir("import").join(format!("plate.{}", format.extension()));
    let options = simple3d_export::Options { format, ..Default::default() };
    simple3d_export::write(&path, mesh, &options, &mut |_| true).expect("the plate is exportable");
    import_file(app, &path);
}

/// Read `path` through the real job, waiting as the frame loop does.
fn import_file(app: &mut App, path: &std::path::Path) {
    app.import_job = Some(ImportJob::spawn(path.to_path_buf(), app.active, std::time::Duration::from_secs(30)));
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while app.import_job.is_some() {
        app.poll_import();
        assert!(std::time::Instant::now() < until, "the import never finished");
        std::thread::yield_now();
    }
}

/// An exported file comes back as a selected body, for every export format.
#[test]
pub(crate) fn every_exported_format_comes_back_as_a_body_in_the_document() {
    for format in simple3d_export::Format::ALL {
        let mut app = app_in(temp_config_dir("import-formats"));
        let root = app.scene.root();
        let source = app.scene.add_primitive("plate", root, 0).expect("the plate is in the registry");
        app.reevaluate_for_test();
        let written = app.evaluated.node_meshes[&source].clone();
        app.scene.remove(source);
        app.reevaluate_for_test();

        import_written(&mut app, &written, format);
        let children = app.scene.node(root).children.clone();
        assert_eq!(children.len(), 1, "{}: the import did not land as one row", format.label());
        let landed = children[0];
        assert!(app.scene.node(landed).is_mesh(), "{}: what landed is not a stored mesh", format.label());
        assert_eq!(app.selection, vec![landed], "{}: the import was not selected", format.label());
        assert_eq!(app.scene.node(landed).name, "plate", "{}: the node is not named after the file", format.label());
        let triangles = app.scene.node(landed).mesh().expect("it is a mesh").triangle_count();
        assert_eq!(triangles, written.weld().triangle_count(), "{}: the geometry changed", format.label());
        assert!(app.status_text().contains("Imported"), "{}: {}", format.label(), app.status_text());
    }
}

/// An import is one undo step, and undoing it restores the document exactly.
#[test]
pub(crate) fn an_import_is_one_undo_step() {
    let mut app = headless_app();
    let before = app.scene.node(app.scene.root()).children.len();
    let steps = app.history.undo_len();
    let mesh = simple3d_geom::primitives::box_mesh(20.0, 20.0, 20.0);
    import_written(&mut app, &mesh, simple3d_export::Format::StlBinary);
    assert_eq!(app.history.undo_len(), steps + 1, "the import was not one step");
    assert_eq!(app.scene.node(app.scene.root()).children.len(), before + 1);

    app.run(Command::Undo);
    assert_eq!(app.scene.node(app.scene.root()).children.len(), before, "undo did not take the import back out");
}

/// A 3MF of several named bodies comes back as those bodies under a group named after the file.
#[test]
pub(crate) fn a_file_of_several_bodies_comes_back_as_a_group_of_them() {
    let mut app = app_in(temp_config_dir("import-bodies"));
    let left = simple3d_geom::primitives::box_mesh(20.0, 20.0, 20.0);
    let right = left.translated(Vec3::new(100.0, 0.0, 0.0));
    let path = temp_config_dir("import-bodies").join("pair.3mf");
    let parts =
        [simple3d_export::Part { name: "Left", mesh: &left }, simple3d_export::Part { name: "Right", mesh: &right }];
    let options = simple3d_export::Options {
        format: simple3d_export::Format::ThreeMf,
        bodies: simple3d_export::BodyMode::TopLevel,
        ..Default::default()
    };
    simple3d_export::write_parts(&path, &parts, &options, &mut |_| true).expect("two boxes are exportable");

    import_file(&mut app, &path);
    let children = app.scene.node(app.scene.root()).children.clone();
    assert_eq!(children.len(), 1, "the bodies should arrive under one row");
    let group = children[0];
    assert_eq!(app.scene.node(group).name, "pair", "the group is not named after the file");
    let names: Vec<String> = app.scene.node(group).children.iter().map(|&id| app.scene.node(id).name.clone()).collect();
    assert_eq!(names, vec!["Left", "Right"], "the bodies came back under different names");
    assert!(app.scene.node(group).children.iter().all(|&id| app.scene.node(id).is_mesh()));
    assert_eq!(app.selection, vec![group], "the group should be what is selected");

    // They came back where they stood, not stacked.
    app.reevaluate_for_test();
    let (lo, hi) = app.evaluated.mesh.bounds().expect("the import evaluated to nothing");
    assert!((hi.x - lo.x - 120.0).abs() < 1e-6, "the two boxes span {} rather than 120", hi.x - lo.x);
}

/// Face-sharing bodies (colours as separate objects) arrive as an assembly, never through the
/// kernel: every triangle kept and no group error.
#[test]
pub(crate) fn bodies_sharing_a_face_arrive_side_by_side() {
    let mut app = app_in(temp_config_dir("import-touching"));
    let left = simple3d_geom::primitives::box_mesh(20.0, 20.0, 20.0);
    let right = left.translated(Vec3::new(20.0, 0.0, 0.0));
    let path = temp_config_dir("import-touching").join("halves.3mf");
    let parts =
        [simple3d_export::Part { name: "Left", mesh: &left }, simple3d_export::Part { name: "Right", mesh: &right }];
    let options = simple3d_export::Options {
        format: simple3d_export::Format::ThreeMf,
        bodies: simple3d_export::BodyMode::TopLevel,
        ..Default::default()
    };
    simple3d_export::write_parts(&path, &parts, &options, &mut |_| true).expect("two boxes are exportable");

    import_file(&mut app, &path);
    let group = app.scene.node(app.scene.root()).children[0];
    assert_eq!(app.scene.node(group).combine_op(), Some(simple3d_core::scene::GroupOp::Assembly));
    app.reevaluate_for_test();
    assert!(app.evaluated.errors.is_empty(), "{:?}", app.evaluated.errors);
    let apart: usize =
        app.scene.node(group).children.iter().map(|&id| app.scene.node(id).mesh().unwrap().triangle_count()).sum();
    assert_eq!(app.evaluated.mesh.triangle_count(), apart, "the halves were combined");
}

/// A non-model file changes nothing and the error says why.
#[test]
pub(crate) fn a_file_that_cannot_be_read_changes_nothing() {
    let mut app = headless_app();
    let before = app.scene.node(app.scene.root()).children.len();
    let steps = app.history.undo_len();
    let path = temp_config_dir("import-broken").join("notes.stl");
    std::fs::write(&path, b"this is not a model at all, whatever it is called").unwrap();

    import_file(&mut app, &path);
    assert_eq!(app.scene.node(app.scene.root()).children.len(), before, "a broken file still added a row");
    assert_eq!(app.history.undo_len(), steps, "a broken file recorded an undo step");
    assert!(matches!(app.modal, Modal::Error), "the failure was not reported");
    assert!(app.error_detail.contains("notes.stl"), "the message does not name the file: {}", app.error_detail);
}

/// A model read for one document never lands in another, even after a tab switch mid-read.
#[test]
pub(crate) fn an_import_read_for_another_document_is_dropped() {
    let mut app = headless_app();
    let before = app.scene.node(app.scene.root()).children.len();
    let path = temp_config_dir("import-tab").join("box.stl");
    let mesh = simple3d_geom::primitives::box_mesh(10.0, 10.0, 10.0);
    let options = simple3d_export::Options { format: simple3d_export::Format::StlBinary, ..Default::default() };
    simple3d_export::write(&path, &mesh, &options, &mut |_| true).unwrap();

    // Read for a tab that is not the one on screen.
    app.import_job = Some(ImportJob::spawn(path, app.active + 1, std::time::Duration::from_secs(30)));
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while app.import_job.is_some() {
        app.poll_import();
        assert!(std::time::Instant::now() < until, "the import never finished");
        std::thread::yield_now();
    }
    assert_eq!(app.scene.node(app.scene.root()).children.len(), before, "the import landed in the wrong document");
    assert!(app.status_text().contains("dropped"), "{}", app.status_text());
}

/// The footer reports the source unit and an open mesh, since exporting it again will be refused.
#[test]
pub(crate) fn the_footer_says_what_arrived_and_what_is_wrong_with_it() {
    let mut app = app_in(temp_config_dir("import-summary"));
    // One triangle: geometry, but not a closed surface.
    let mut open = simple3d_geom::Mesh::new();
    open.push_triangle(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), Vec3::new(0.0, 10.0, 0.0));
    let model = simple3d_import::Model {
        format: simple3d_import::Format::Stl,
        unit: None,
        parts: vec![simple3d_import::Part { name: String::new(), mesh: open }],
    };
    app.place_import("sheet", model);
    let said = app.status_text();
    assert!(said.contains("1 triangle") && said.contains("STL"), "{said}");
    assert!(said.contains("not a closed solid"), "{said}");

    let mut app = app_in(temp_config_dir("import-summary-unit"));
    let model = simple3d_import::Model {
        format: simple3d_import::Format::ThreeMf,
        unit: Some(simple3d_import::Unit::Inch),
        parts: vec![simple3d_import::Part {
            name: String::new(),
            mesh: simple3d_geom::primitives::box_mesh(25.4, 25.4, 25.4),
        }],
    };
    app.place_import("bracket", model);
    assert!(app.status_text().contains("converted from inches"), "{}", app.status_text());
}

/// The command is bound. No test runs `Command::Import` itself, since that opens a real portal
/// dialog on the tester's screen; `file_dialog.rs` drives the answer side with its own channel.
#[test]
pub(crate) fn the_import_command_is_bound_in_every_preset() {
    use simple3d_core::keymap::{Keymap, Preset};
    for preset in Preset::ALL {
        let map = Keymap::from_preset(preset);
        assert!(map.binding(Command::Import).is_some(), "{preset:?} does not bind the import");
        assert!(map.self_conflicts().is_empty(), "{preset:?}: {:?}", map.self_conflicts());
    }
}
