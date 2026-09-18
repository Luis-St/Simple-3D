//! Putting a mesh back into objects and groups (issue 108).

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::scene::NodeId;
use simple3d_geom::reassemble::Reassemble;
use std::time::{Duration, Instant};

/// A document holding one mesh node: a plate with a pin standing on it, as two
/// shells in one bag of triangles.
///
/// Built by appending the two solids rather than by unioning them, because that
/// is what an imported assembly *is*: a printer file holds every body's surface
/// side by side and nothing in it says they are one solid. Putting the two
/// through the boolean kernel first would weld them into one surface, which is
/// a different object and one the recognition is right to refuse.
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

/// Wait for the run the tool has started, the way the frame loop does: the
/// analysis is on a thread precisely so the window does not wait for it, so a
/// test has to.
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
fn open_with(app: &mut App, plan: Reassemble) {
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
    // A box and a cylinder, both recognised, both with their parameters back.
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

/// The one thing that has to be exactly right: a pin that comes back somewhere
/// other than where it stood is a reassembly of a different model.
///
/// Measured one way round, and that is not a weaker test than it looks. The
/// objects are combined by a union, which takes out the faces where the pin
/// meets the plate -- the pin's own bottom cap is inside the solid now, and was
/// a surface in the mesh, so measuring the mesh against the result finds it and
/// calls the difference three millimetres. What the result may not have is a
/// surface anywhere the mesh had none, and a shape placed even slightly off
/// has one everywhere.
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

/// Nothing goes into the document until Reassemble is pressed. The window is
/// open, a run has landed, and the tree is still the one mesh it was.
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
    // Two objects straight under the reassembled node, and no group between.
    assert_eq!(app.scene.node(id).children.len(), 2);
    assert!(app.scene.node(id).children.iter().all(|&child| !app.scene.node(child).is_group()));
}

/// The cap is the setting the whole feature needs, and it has to hold: what is
/// past it stays in one mesh rather than becoming a node apiece.
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
    // One group in the whole assembly is the reassembled node itself, so the
    // two bodies stand straight under it rather than inside a second group.
    assert_eq!(kinds(&app, id), vec!["mesh".to_string(), "mesh".to_string()]);
}

/// A mesh that is one body and nothing recognisable is the mesh it already is,
/// and the tool says so rather than offering to wrap it in a group.
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

/// A run in flight has to keep the frames coming: it hands its answer back over
/// a channel, which is not an event the toolkit knows about, so a frame loop
/// that goes to sleep after the number was typed never shows what it did.
#[test]
fn a_run_in_flight_keeps_the_frames_coming() {
    let (mut app, _) = app_with_an_assembly("reassemble-frames");
    app.run(Command::Reassemble);
    app.refresh_reassemble_tool();
    assert!(app.reassemble_tool.as_ref().unwrap().job.is_some(), "no run was started");
    assert!(app.work_in_flight(), "the loop would go to sleep with the answer still to come");
}

/// The viewport keeps its last image while nothing that went into it has
/// changed, so anything that *does* change it has to be part of the key that
/// image is kept under -- and what was found is drawn over the mesh rather than
/// being the mesh, so nothing else in the key moves with it.
#[test]
fn turning_the_preview_off_redraws_the_viewport() {
    let (mut app, _) = app_with_an_assembly("reassemble-key");
    open_with(&mut app, Reassemble::default());
    let size = [800, 600];
    let before = crate::panel_viewport::image_key(&app, size, true);

    app.reassemble_tool.as_mut().unwrap().outlines = false;
    assert_ne!(crate::panel_viewport::image_key(&app, size, true), before, "the picture would not be drawn again");
}

/// What is drawn lands on the model rather than beside it: the bodies are found
/// in the mesh's own frame, and the node it is on has a transform.
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

/// The window draws, with every field and the summary in it, on a real frame.
#[test]
fn the_window_draws() {
    let (mut app, _) = app_with_an_assembly("reassemble-window");
    app.run(Command::Reassemble);
    draw_one_frame(&mut app);
    assert!(app.reassemble_tool.is_some(), "drawing the window closed it");
}

/// A recognised sphere still draws something.
///
/// Every other shape is drawn by its corners, and a finely tessellated sphere
/// has none: its facets meet at a few degrees each. Asking for its corners asks
/// for nothing, and a sphere that came back recognised but drawn as *nothing at
/// all* reads, in a viewport where everything else is outlined, as a body the
/// tool missed.
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
