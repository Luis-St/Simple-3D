//! Adding a pattern, and filling an empty one.

use super::*;
use simple3d_core::keymap::Command;
use simple3d_core::scene::GroupOp;

#[test]
pub(crate) fn adding_from_a_patterns_own_row_puts_the_shape_inside_it() {
    // Issue 67: the outliner's row menu asked `is_group`, so Add from a
    // pattern's row dropped the shape beside the pattern -- disagreeing with
    // the drag-and-drop rule and with the document-level Add, both of which
    // already put it in.
    let mut app = headless_app();
    app.run(Command::Pattern);
    let pat = app.primary().unwrap();
    let before = app.scene.node(pat).children.len();
    let root_before = app.scene.node(app.scene.root()).children.len();

    app.add_node_at(pat, Some("box"), GroupOp::Union);
    assert_eq!(app.scene.node(pat).children.len(), before + 1, "the shape did not go into the pattern");
    assert_eq!(app.scene.node(app.scene.root()).children.len(), root_before, "it landed beside the pattern");

    // And the document-level Add agrees, as it already did.
    assert_eq!(app.scene.insertion_point(Some(pat)).0, pat);
}

#[test]
pub(crate) fn a_pattern_made_empty_is_sized_by_the_first_shape_added_into_it() {
    // The pattern *tool* measures the shapes it wraps, so Ctrl+Shift+P on a
    // 20 mm box gives a 30 mm step. A pattern made with nothing selected
    // has nothing to measure yet and kept the stock 20 mm, so a 20 mm shape
    // added into it afterwards was repeated at exactly its own width and
    // the copies fused into one bar instead of standing clear.
    let mut app = app_in(temp_config_dir("pattern-sized-on-add"));
    app.clear_selection();
    app.run(Command::Pattern);
    let pat = app.primary().expect("the pattern is selected");
    assert_eq!(app.scene.node(pat).params(), Some(&simple3d_core::pattern::default_params()));

    // Through the palette's own path -- a click on a tile with the empty
    // pattern selected -- rather than the outliner's row menu.
    app.add_node(Some("box"), GroupOp::Union);
    let params = app.scene.node(pat).params().expect("the pattern kept its parameters");
    assert_ne!(params, &simple3d_core::pattern::default_params(), "the pattern kept the stock 20 mm step");

    // And what it evaluates to is three shapes standing clear, not one bar.
    app.reevaluate_for_test();
    let (lo, hi) = app.evaluated.node_meshes[&pat].bounds().expect("the pattern evaluated to nothing");
    assert!(hi.x - lo.x > 60.0, "the copies fused: the run is only {} mm across", hi.x - lo.x);
}

#[test]
pub(crate) fn an_empty_pattern_can_be_added_the_way_a_group_is() {
    // Both Add menus offer a pattern beside the group operators (issue 67).
    // It is the opposite gesture to Ctrl+Shift+P, which wraps the selection:
    // this always makes an empty one to fill afterwards, whatever is
    // selected.
    let mut app = headless_app();
    let plate = app.primary().unwrap();
    let root = app.scene.root();

    // From a plain row: beside it, not inside it, and left selected.
    app.add_pattern_at(plate);
    let beside = app.primary().unwrap();
    assert!(app.scene.node(beside).is_pattern());
    assert!(app.scene.node(beside).children.is_empty(), "it wrapped the selection instead of being empty");
    assert_eq!(app.scene.node(beside).parent, Some(root));

    // From a pattern's own row: into it, the same rule Add already follows.
    app.add_pattern_at(beside);
    let inside = app.primary().unwrap();
    assert_eq!(app.scene.node(inside).parent, Some(beside));

    // And the menu bar's Add, which uses the document's insertion point.
    app.select_only(plate);
    app.add_pattern();
    let added = app.primary().unwrap();
    assert!(app.scene.node(added).is_pattern());
    assert!(app.scene.node(added).children.is_empty());
    // One undo step takes it back, and the plate is untouched by all of it.
    app.run(Command::Undo);
    assert!(!app.scene.contains(added));
    assert!(app.scene.contains(plate));
}

#[test]
pub(crate) fn a_pattern_made_empty_is_measured_the_moment_it_gains_a_shape() {
    // Issue 67, from the open list: the tool sizes the spacing to the shapes
    // it wraps, but a pattern made with nothing selected has nothing to
    // measure and kept the stock 20 mm step -- exactly the width of the stock
    // box -- so a box dropped into it afterwards was repeated face to face.
    use simple3d_core::primitive::ParamsExt;
    let mut app = headless_app();
    app.clear_selection();
    app.run(Command::Pattern);
    let pat = app.primary().unwrap();
    assert!(app.scene.node(pat).children.is_empty(), "the tool made a pattern with something in it");
    let stock = app.scene.node(pat).params().unwrap().num("step_x");

    app.add_node_at(pat, Some("box"), GroupOp::Union);
    let step = app.scene.node(pat).params().unwrap().num("step_x");
    let width = simple3d_core::eval::subtree_bounds(&app.scene, app.scene.node(pat).children[0])
        .map(|(lo, hi)| hi.x - lo.x)
        .unwrap();
    assert!(step > width, "a {width}mm shape is repeated at {step}mm, so its copies touch or overlap");
    assert!((step - stock).abs() > 1e-9, "the spacing was left at the stock number");
    app.reevaluate_for_test();
    assert!(app.evaluated.errors.is_empty(), "{:?}", app.evaluated.errors);

    // But numbers the user has settled are never overwritten: a second shape
    // dropped in leaves the spacing exactly as it stands.
    app.add_node_at(pat, Some("box"), GroupOp::Union);
    assert!((app.scene.node(pat).params().unwrap().num("step_x") - step).abs() < 1e-9);
}

#[test]
pub(crate) fn the_pattern_tool_spaces_its_copies_clear_of_the_shapes_it_wraps() {
    // Regression, issue 67: the stock 20 mm step is exactly the stock box's
    // width, so the tool's own output was three copies face to face -- and
    // the scene went red on the feature's very first use. The spacing now
    // comes from what is being wrapped, whatever size that is.
    for (w, d, h) in [(20.0, 20.0, 20.0), (4.0, 4.0, 4.0), (120.0, 60.0, 8.0)] {
        let mut app = headless_app();
        let root = app.scene.root();
        let id = app.scene.add_primitive("box", root, 0).unwrap();
        {
            let params = app.scene.get_mut(id).unwrap().params_mut().unwrap();
            params.insert("width".into(), simple3d_core::primitive::ParamValue::Length(w));
            params.insert("depth".into(), simple3d_core::primitive::ParamValue::Length(d));
            params.insert("height".into(), simple3d_core::primitive::ParamValue::Length(h));
        }
        app.select_only(id);
        app.run(Command::Pattern);
        let pat = app.primary().unwrap();
        let step = linear_run(&app, pat);
        assert!(step.x > w, "a {w}mm shape got a {}mm step, so its copies touch or overlap", step.x);
        // Scaled to the shape itself, not to what the pattern makes of it:
        // measuring the node *after* it became a pattern measured three
        // copies rather than one and inflated every distance threefold.
        assert!(
            (step.x - w * 1.5).abs() < 1e-6,
            "a {w}mm shape got a {}mm step; the spacing was taken from the repetition, not the shape",
            step.x
        );
        app.reevaluate_for_test();
        assert!(app.evaluated.errors.is_empty(), "{w}x{d}x{h}: {:?}", app.evaluated.errors);
    }
}
