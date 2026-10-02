//! Putting a mesh back into objects and groups (issue 108).

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::scene::NodeId;
use simple3d_geom::reassemble::Reassemble;
use std::time::{Duration, Instant};

/// A document with one mesh node: a plate and a pin as two shells in one bag of triangles.
/// Appended rather than unioned, as in an imported assembly; a union would weld them into one.
fn app_with_an_assembly(name: &str) -> (App, NodeId) {
    use simple3d_geom::primitives as gen;
    let mut app = app_in(temp_config_dir(name));
    let root = app.scene.root();
    let mut mesh = gen::box_mesh(40.0, 40.0, 4.0);
    mesh.append(&gen::cylinder_mesh(6.0, 6.0, 20.0, 24).translated(simple3d_geom::Vec3::new(0.0, 0.0, 12.0)));
    let id = app.scene.add_mesh("Assembly", simple3d_core::mesh_data::MeshData::new(mesh), root, 0);
    app.select_only(id);
    app.reevaluate_for_test();
    app.history.clear();
    app.saved_revision = app.history.revision();
    (app, id)
}

/// Wait for the tool's background run, as the frame loop would.
fn wait_for_answer(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        app.refresh_reassemble_tool();
        let tool = app.reassemble_tool.as_ref().expect("the tool is open");
        if tool.found.as_ref().is_some_and(|found| found.plan == tool.plan) {
            return;
        }
        assert!(Instant::now() < deadline, "the reassembly never finished");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Open the tool on the selection and set its numbers.
pub(super) fn open_with(app: &mut App, plan: Reassemble) {
    app.run(Command::Reassemble);
    app.reassemble_tool.as_mut().expect("the tool opened on the selection").plan = plan;
    wait_for_answer(app);
}

/// The kinds of node under `id`, deepest first, as a sorted list of labels.
fn kinds(app: &App, id: NodeId) -> Vec<String> {
    let mut out: Vec<String> = app
        .scene
        .descendants(id)
        .into_iter()
        .map(|child| match app.scene.node(child).spec() {
            Some(spec) => spec.type_id.to_string(),
            None => app.scene.node(child).kind_label().to_string(),
        })
        .collect();
    out.sort();
    out
}

#[test]
fn a_baked_assembly_comes_back_as_objects() {
    let (mut app, id) = app_with_an_assembly("reassemble");
    assert!(app.scene.node(id).is_mesh(), "the fixture is not a mesh");

    open_with(&mut app, Reassemble::default());
    app.apply_reassemble();

    assert!(app.reassemble_tool.is_none(), "the window stayed open");
    assert!(app.scene.node(id).is_group(), "the mesh did not become a group");
    assert_eq!(kinds(&app, id), vec!["box".to_string(), "cylinder".to_string()]);
    let box_id = app
        .scene
        .descendants(id)
        .into_iter()
        .find(|&child| app.scene.node(child).spec().is_some_and(|spec| spec.type_id == "box"))
        .expect("the plate came back as a box");
    let params = app.scene.node(box_id).params().expect("a box has parameters");
    use simple3d_core::primitive::ParamsExt;
    assert!((params.num("width") - 40.0).abs() < 1e-3, "width {}", params.num("width"));
    assert!((params.num("height") - 4.0).abs() < 1e-3, "height {}", params.num("height"));
}

#[test]
fn the_node_keeps_its_name_and_its_place() {
    let (mut app, id) = app_with_an_assembly("reassemble-name");
    let name = app.scene.node(id).name.clone();
    let parent = app.scene.node(id).parent;

    open_with(&mut app, Reassemble::default());
    app.apply_reassemble();

    assert_eq!(app.scene.node(id).name, name, "the group is a different node wearing its name");
    assert_eq!(app.scene.node(id).parent, parent);
    assert_eq!(app.selection, vec![id], "the reassembled group is not what is selected");
}

/// The pin must come back exactly where it stood.
///
/// Measured one way only: the union removes the pin's buried bottom cap, so mesh-to-result
/// differs by 3 mm, but a misplaced shape would add surface where the mesh had none.
#[test]
fn the_shape_lands_where_the_triangles_were() {
    let (mut app, _) = app_with_an_assembly("reassemble-place");
    app.reevaluate_for_test();
    let before = app.evaluated.mesh.clone();

    open_with(&mut app, Reassemble::default());
    app.apply_reassemble();
    app.reevaluate_for_test();

    let moved = simple3d_geom::simplify::measure::furthest_from(&app.evaluated.mesh.positions, &before);
    assert!(moved < 1e-3, "the objects came back {moved} mm from where the triangles were");
}

#[test]
fn keeping_the_result_is_one_undo_step_back_to_the_mesh() {
    let (mut app, id) = app_with_an_assembly("reassemble-undo");
    open_with(&mut app, Reassemble::default());
    app.apply_reassemble();
    assert_eq!(app.history.undo_len(), 1, "taking a mesh apart should be one step");

    app.run(Command::Undo);
    assert!(app.scene.node(id).is_mesh(), "undo did not bring the mesh back");
    assert!(app.scene.node(id).children.is_empty(), "undo left the objects behind");
}

#[test]
fn cancelling_changes_nothing() {
    let (mut app, id) = app_with_an_assembly("reassemble-cancel");
    open_with(&mut app, Reassemble::default());
    app.cancel_reassemble_tool();

    assert!(app.reassemble_tool.is_none());
    assert!(app.scene.node(id).is_mesh(), "the mesh was taken apart anyway");
    assert_eq!(app.history.undo_len(), 0, "cancelling left an undo step behind");
}

/// Nothing enters the document until Reassemble is pressed.
#[test]
fn the_document_is_untouched_while_the_window_is_open() {
    let (mut app, id) = app_with_an_assembly("reassemble-open");
    open_with(&mut app, Reassemble::default());

    assert!(app.scene.node(id).is_mesh());
    assert_eq!(app.history.undo_len(), 0);
    assert!(!app.reassemble_tool.as_ref().unwrap().found.as_ref().unwrap().assembly.parts.is_empty());
}

#[test]
fn bodies_that_touch_are_grouped_only_when_asked() {
    let (mut app, id) = app_with_an_assembly("reassemble-group");
    open_with(&mut app, Reassemble { group_touching: false, ..Reassemble::default() });
    app.apply_reassemble();
    // Two objects directly under the reassembled node, with no group between.
    assert_eq!(app.scene.node(id).children.len(), 2);
    assert!(app.scene.node(id).children.iter().all(|&child| !app.scene.node(child).is_group()));
}

/// Bodies past the cap stay in one mesh rather than becoming nodes.
#[test]
fn the_cap_holds_and_the_rest_is_kept_as_one_mesh() {
    let (mut app, id) = app_with_an_assembly("reassemble-cap");
    open_with(&mut app, Reassemble { max_objects: 1, group_touching: false, ..Reassemble::default() });
    app.apply_reassemble();

    let children = app.scene.node(id).children.clone();
    assert_eq!(children.len(), 2, "one object and the mesh holding what was left");
    assert!(app.scene.node(children[1]).is_mesh(), "what was past the cap is not a mesh");
    assert!(app.scene.node(children[1]).name.contains("Remainder"));
}

#[test]
fn recognition_can_be_turned_off_to_leave_the_bodies_as_meshes() {
    let (mut app, id) = app_with_an_assembly("reassemble-raw");
    open_with(&mut app, Reassemble { recognise: false, ..Reassemble::default() });
    app.apply_reassemble();
    // The reassembled node is the only group, so both bodies sit directly under it.
    assert_eq!(kinds(&app, id), vec!["mesh".to_string(), "mesh".to_string()]);
}

/// A single unrecognisable body stays as it is, and the tool says so.
#[test]
fn a_mesh_with_nothing_in_it_is_not_offered() {
    let mut app = app_in(temp_config_dir("reassemble-nothing"));
    let root = app.scene.root();
    let id = app.scene.add_primitive("torus", root, 0).expect("the torus is in the registry");
    app.select_only(id);
    app.reevaluate_for_test();
    app.run(Command::ConvertToMesh);
    app.reevaluate_for_test();

    open_with(&mut app, Reassemble::default());
    assert!(!app.reassemble_ready(), "a mesh that came back a mesh was offered as a reassembly");
    app.apply_reassemble();
    assert!(app.scene.node(id).is_mesh(), "it was taken apart anyway");
}

#[test]
fn only_a_mesh_can_be_reassembled() {
    let mut app = headless_app();
    app.run(Command::Reassemble);
    assert!(app.reassemble_tool.is_none(), "the tool opened on a primitive");
    assert!(app.status_text().contains("mesh"), "the refusal does not say what it wants: {}", app.status_text());
}

/// A run in flight keeps requesting frames: its channel answer is not a toolkit event, so a
/// sleeping frame loop would never show it.
#[test]
fn a_run_in_flight_keeps_the_frames_coming() {
    let (mut app, _) = app_with_an_assembly("reassemble-frames");
    app.run(Command::Reassemble);
    app.refresh_reassemble_tool();
    assert!(app.reassemble_tool.as_ref().unwrap().job.is_some(), "no run was started");
    assert!(app.work_in_flight(), "the loop would go to sleep with the answer still to come");
}

/// The found overlay must be part of the viewport's image cache key, since nothing else in the
/// key changes with it.
#[test]
fn turning_the_preview_off_redraws_the_viewport() {
    let (mut app, _) = app_with_an_assembly("reassemble-key");
    open_with(&mut app, Reassemble::default());
    let size = [800, 600];
    let before = crate::panel_viewport::image_key(&app, size, true);

    app.reassemble_tool.as_mut().unwrap().outlines = false;
    assert_ne!(crate::panel_viewport::image_key(&app, size, true), before, "the picture would not be drawn again");
}

/// The overlay lands on the model: bodies are found in the mesh's frame and the node is transformed.
#[test]
fn what_is_drawn_stands_on_the_model() {
    let (mut app, id) = app_with_an_assembly("reassemble-draw");
    app.scene.get_mut(id).expect("it is there").position = simple3d_geom::Vec3::new(100.0, 0.0, 0.0);
    app.reevaluate_for_test();
    open_with(&mut app, Reassemble::default());

    let loops = crate::reassemble_tool::preview_loops(&app);
    assert!(!loops.is_empty(), "nothing was drawn");
    let x = loops.iter().flatten().map(|p| p.x).fold(f64::MIN, f64::max);
    assert!(x > 50.0, "what was found is drawn at x={x}, back where the mesh is not");
}

#[test]
fn leaving_the_tab_puts_the_tool_away() {
    let (mut app, _) = app_with_an_assembly("reassemble-tab");
    open_with(&mut app, Reassemble::default());
    app.new_project();
    app.activate_tab(0);
    assert!(app.reassemble_tool.is_none(), "the tool followed the document it was not about");
}

/// The window draws with every field and the summary on a real frame.
#[test]
fn the_window_draws() {
    let (mut app, _) = app_with_an_assembly("reassemble-window");
    app.run(Command::Reassemble);
    draw_one_frame(&mut app);
    assert!(app.reassemble_tool.is_some(), "drawing the window closed it");
}

/// A recognised sphere still draws something: it has no corners to outline, and drawing nothing
/// looks like a missed body.
#[test]
fn a_sphere_is_still_drawn() {
    let mut app = app_in(temp_config_dir("reassemble-sphere"));
    let root = app.scene.root();
    let mesh = simple3d_geom::primitives::ellipsoid_mesh(30.0, 30.0, 30.0, 48);
    let id = app.scene.add_mesh("Ball", simple3d_core::mesh_data::MeshData::new(mesh), root, 0);
    app.select_only(id);
    app.reevaluate_for_test();

    open_with(&mut app, Reassemble::default());
    let found = app.reassemble_tool.as_ref().unwrap().found.as_ref().unwrap();
    assert_eq!(found.assembly.recognised(), 1, "the sphere was not recognised, so this tests nothing");
    assert!(!crate::reassemble_tool::preview_loops(&app).is_empty(), "a recognised sphere drew nothing");
}
