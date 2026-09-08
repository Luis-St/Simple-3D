//! What a tool's preview shows, and what it hides.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::primitive::ParamValue;
use simple3d_core::scene::GroupOp;
use simple3d_geom::Vec3;

#[test]
pub(crate) fn a_preview_only_reaches_the_viewport_while_a_tool_is_open() {
    // The setting says what the viewport does *while a preview is up*, and
    // nothing at all otherwise: it is not a second way to turn the grid off.
    let mut app = headless_app();
    app.scene.settings.preview_viewport = simple3d_core::scene::PreviewViewport::PreviewOnly;
    assert_eq!(app.preview_subject(), None, "something claims to be previewing with no tool open");

    let plate = app.primary().unwrap();
    app.reevaluate_for_test();
    app.open_split_tool();
    assert_eq!(app.preview_subject(), Some(plate));

    // A shape deleted under the tool takes the preview with it rather than
    // leaving the viewport emptied for an object that is gone.
    app.scene.remove(plate);
    assert_eq!(app.preview_subject(), None);
}

#[test]
pub(crate) fn what_the_viewport_does_under_a_preview_is_saved_with_the_document() {
    // It is a document setting, not a preference: which of the grid, the
    // axes and the rest of the scene is in the way is a property of what is
    // being modelled (issue 82).
    use simple3d_core::scene::PreviewViewport;
    let mut app = headless_app();
    assert_eq!(app.scene.settings.preview_viewport, PreviewViewport::NoChange, "the default is not no change");
    // The default is absent from the file, so a project that never touched
    // this still diffs cleanly against one written before it existed.
    assert!(!simple3d_core::project::to_string(&app.scene).contains("preview_viewport"));

    app.scene.settings.preview_viewport = PreviewViewport::PreviewOnly;
    let text = simple3d_core::project::to_string(&app.scene);
    let back = simple3d_core::project::from_str(&text).expect("it loads");
    assert_eq!(back.settings.preview_viewport, PreviewViewport::PreviewOnly);
}

#[test]
pub(crate) fn each_preview_mode_hides_exactly_what_it_names() {
    use simple3d_core::scene::PreviewViewport::*;
    // Read as a table, because the five are only ever right together: a
    // mode that hides one thing too many is a viewport with the ground gone
    // for no reason the user asked for.
    for (mode, grid, axes, others) in [
        (NoChange, true, true, true),
        (HideAxes, true, false, true),
        (HideGrid, false, true, true),
        (HideGridAndAxes, false, false, true),
        (PreviewOnly, false, false, false),
    ] {
        assert_eq!(mode.keeps_grid(), grid, "{mode:?} grid");
        assert_eq!(mode.keeps_axes(), axes, "{mode:?} axes");
        assert_eq!(mode.keeps_other_bodies(), others, "{mode:?} other bodies");
    }
}

#[test]
pub(crate) fn the_previewed_object_has_a_renderable_even_when_it_is_not_selected() {
    // "Only what is previewed" draws that object as the model, so it needs
    // a renderable of its own -- and the selection can move on to something
    // else while the tool is open, which is what used to take it away.
    let mut app = headless_app();
    let plate = app.primary().unwrap();
    app.reevaluate_for_test();
    app.open_split_tool();
    let root = app.scene.root();
    app.select_only(root);
    app.refresh_node_renderables();
    assert!(app.node_renderables.contains_key(&plate), "the previewed object has nothing to draw");
}

#[test]
pub(crate) fn the_split_tool_rebakes_when_the_shape_changes_under_it() {
    // The window is not modal, so the shape it is cutting can be edited
    // while it is open -- and a plan drawn over the shape as it was is a
    // plan of cuts that will not fall there (issue 82).
    let mut app = headless_app();
    let plate = app.primary().unwrap();
    app.reevaluate_for_test();
    app.open_split_tool();
    let was = app.split_tool.as_ref().expect("the tool opened").bounds;

    app.scene.get_mut(plate).unwrap().params_mut().unwrap().insert("width".into(), ParamValue::Length(120.0));
    app.reevaluate_for_test();
    app.refresh_split_tool();
    let now = app.split_tool.as_ref().expect("the tool is still open").bounds;
    assert!((now.1.x - was.1.x).abs() > 1.0, "the tool is still drawing the shape as it was: {was:?} -> {now:?}");

    // And a shape deleted under it closes the tool rather than leaving a
    // window open on nothing.
    app.scene.remove(plate);
    app.refresh_split_tool();
    assert!(app.split_tool.is_none(), "the tool stayed open on an object that is gone");
}

#[test]
pub(crate) fn joining_something_that_was_never_split_says_so_rather_than_working() {
    let mut app = headless_app();
    let before = app.history.undo_len();
    app.run(Command::Rejoin);
    assert_eq!(app.history.undo_len(), before, "joining a shape that is not a split recorded an undo step");
    assert!(app.status_text().contains("split into pieces"), "{}", app.status_text());
}

#[test]
pub(crate) fn a_split_survives_saving_and_loading_with_the_object_it_was_made_from() {
    // The split is a body of its own in the project file (format 3), and the
    // recipe it holds has to come back with it or the break stops being
    // reversible the moment the file is closed.
    let mut app = headless_app();
    let root = app.scene.root();
    let group = app.scene.add_group(GroupOp::Union, root, 0);
    for i in 0..2 {
        let id = app.scene.add_primitive("box", group, i).unwrap();
        app.scene.get_mut(id).unwrap().position = Vec3::new(i as f64 * 60.0, 0.0, 0.0);
    }
    app.select_only(group);
    app.reevaluate_for_test();
    split_with(&mut app, simple3d_geom::tiling::Tiling { size: 40.0, ..Default::default() });
    let pieces = app.scene.node(app.primary().unwrap()).children.len();

    let text = simple3d_core::project::to_string(&app.scene);
    let scene = simple3d_core::project::from_str(&text).expect("a scene with a split loads");
    let split = scene.depth_first().into_iter().find(|&id| scene.node(id).is_split()).expect("the split is there");
    assert_eq!(scene.node(split).children.len(), pieces, "the pieces did not survive the file");
    let original = scene.node(split).split_original().expect("the object it was made from");
    assert_eq!(original.type_id, "group");
    assert_eq!(original.children.len(), 2, "the operands were not written to the file");

    // And it still joins back together after the round trip.
    let mut reopened = headless_app();
    reopened.scene = scene;
    let restored = reopened.scene.restore_split(split).expect("the recipe rebuilds");
    assert_eq!(reopened.scene.node(restored).group_op(), Some(GroupOp::Union));
    assert_eq!(reopened.scene.node(restored).children.len(), 2);
}
