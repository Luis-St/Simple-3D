//! Bringing a model file into the document (issue 105): what it lands as, what
//! it says, and the cases where nothing is brought in at all.

use super::*;
use crate::worker::ImportJob;
use simple3d_core::keymap::Command;
use simple3d_geom::Vec3;

/// Write `mesh` out in `format`, then import the file the application's own
/// way: the job on its thread, and `poll_import` taking the answer.
fn import_written(app: &mut App, mesh: &simple3d_geom::Mesh, format: simple3d_export::Format) {
    let path = temp_config_dir("import").join(format!("plate.{}", format.extension()));
    let options = simple3d_export::Options { format, ..Default::default() };
    simple3d_export::write(&path, mesh, &options, &mut |_| true).expect("the plate is exportable");
    import_file(app, &path);
}

/// Read `path` through the real job, waiting for the thread the way the
/// application's frame loop does.
fn import_file(app: &mut App, path: &std::path::Path) {
    app.import_job = Some(ImportJob::spawn(path.to_path_buf(), app.active, std::time::Duration::from_secs(30)));
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while app.import_job.is_some() {
        app.poll_import();
        assert!(std::time::Instant::now() < until, "the import never finished");
        std::thread::yield_now();
    }
}

/// The issue itself, at the level the user meets it: a file the application
/// exported comes back as a body in the document, selected and ready to be
/// moved -- for every format the export dialog offers.
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

/// An import is one undo step, and undoing it leaves the document exactly as
/// it was -- the same guarantee every other edit gives.
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

/// A 3MF written as several named bodies comes back as those bodies, under a
/// group named after the file: the rows that were exported are the rows that
/// come back, rather than one merged bag of triangles.
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

    // And they came back where they stood, not on top of one another.
    app.reevaluate_for_test();
    let (lo, hi) = app.evaluated.mesh.bounds().expect("the import evaluated to nothing");
    assert!((hi.x - lo.x - 120.0).abs() < 1e-6, "the two boxes span {} rather than 120", hi.x - lo.x);
}

/// A file that is not a model changes nothing: the error says what is wrong,
/// and the document is left alone rather than gaining an empty row.
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

/// A model read for one document is never dropped into another. The file is
/// chosen and read over several frames, and the user may switch tabs in the
/// meantime -- the import belongs to the document that asked for it.
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

/// What the footer says. A model in another unit says what it was converted
/// from, and one that is not a closed solid says so -- an import cannot refuse
/// a mesh somebody else wrote, but exporting it again will be refused, and that
/// is worth knowing when it arrives rather than then.
#[test]
pub(crate) fn the_footer_says_what_arrived_and_what_is_wrong_with_it() {
    let mut app = app_in(temp_config_dir("import-summary"));
    // One triangle: geometry, and nowhere near a closed surface.
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

/// The command is bound, so the menu entry and the shortcut both reach the
/// import.
///
/// Nothing here runs `Command::Import` itself, and no test may: the dispatch
/// puts a *real* file dialog up through the desktop's portal, on the screen of
/// whoever is running the tests. `file_dialog.rs` drives the answer side by
/// building a `FilePrompt` with a channel of its own, for the same reason.
#[test]
pub(crate) fn the_import_command_is_bound_in_every_preset() {
    use simple3d_core::keymap::{Keymap, Preset};
    for preset in Preset::ALL {
        let map = Keymap::from_preset(preset);
        assert!(map.binding(Command::Import).is_some(), "{preset:?} does not bind the import");
        assert!(map.self_conflicts().is_empty(), "{preset:?}: {:?}", map.self_conflicts());
    }
}
