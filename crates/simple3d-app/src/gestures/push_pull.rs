//! The push/pull tool's drag (issue 73).

use super::*;
use simple3d_core::scene::Placing;
use simple3d_geom::Vec3;

/// The plate's top face centre on screen.
fn top_of_plate(harness: &Harness<'_, App>) -> egui::Pos2 {
    let app = harness.state();
    let (lo, hi) = app.evaluated.bounds.unwrap();
    let top = Vec3::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0, hi.z);
    app.current_view().project(top).unwrap().0
}

#[test]
pub(crate) fn dragging_a_face_up_pushes_it_out_of_its_object_and_down_pulls_it_in() {
    let mut harness = harness("push-pull");
    harness.state_mut().run(simple3d_core::keymap::Command::ModePushPull);
    harness.step();
    let plate = harness.state().scene.node(harness.state().scene.root()).children[0];
    let at = top_of_plate(&harness);
    drag(&mut harness, at, at - egui::vec2(0.0, 60.0), 6);

    let app = harness.state();
    assert_eq!(app.selection.last(), Some(&plate), "the pushed object is not selected");
    assert_eq!(app.scene.node(app.scene.root()).children, vec![plate], "the push made a node of its own");
    let edits = &app.scene.node(plate).edits;
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].as_push().unwrap().placing, Placing::Add);
    let step = app.scene.settings.snap_step;
    let distance = edits[0].size();
    assert!(distance > 0.0 && ((distance / step).round() * step - distance).abs() < 1e-9, "distance {distance}");
    assert_eq!(app.history.undo_len(), 1, "the push is not one undo step");

    // Evaluated again, a downward drag on the pushed top pulls a cut out of the same plate.
    harness.state_mut().evaluated = Evaluator::new().evaluate(&harness.state().scene, &Cancel::new());
    harness.step();
    let at = top_of_plate(&harness);
    drag(&mut harness, at, at + egui::vec2(0.0, 20.0), 6);
    let app = harness.state();
    assert_eq!(app.scene.node(app.scene.root()).children, vec![plate]);
    assert_eq!(app.scene.node(plate).edits.len(), 2);
    assert_eq!(app.scene.node(plate).edits[1].as_push().unwrap().placing, Placing::Cut);
}

#[test]
pub(crate) fn escape_lets_go_of_a_face_without_changing_anything() {
    let mut harness = harness("push-pull-escape");
    harness.state_mut().run(simple3d_core::keymap::Command::ModePushPull);
    harness.step();
    let at = top_of_plate(&harness);
    press(&mut harness, at);
    for k in 1..=4 {
        move_to(&mut harness, at - egui::vec2(0.0, 10.0 * k as f32));
    }
    let drag = harness.state().push_pull.drag.as_ref().expect("the press on the face did not take hold of it");
    // The swept solid is drawn by the renderer, green and depth-tested, rather than over the picture.
    assert!(drag.prism.is_some(), "the drag built no solid to draw");
    let palette = crate::render::Palette::dark();
    let template = crate::push_pull_tool::template(&harness.state().push_pull, &palette);
    assert_eq!(template.and_then(|(_, _, colour)| colour), Some(palette.extrusion));
    key(&mut harness, egui::Key::Escape);
    release(&mut harness, at - egui::vec2(0.0, 40.0));
    assert!(harness.state().push_pull.drag.is_none());
    assert_eq!(harness.state().scene.node(harness.state().scene.root()).children.len(), 1);
}

#[test]
pub(crate) fn the_panel_extracts_a_push_as_a_node_and_removes_a_pull() {
    use egui_kittest::kittest::Queryable;
    let mut harness = harness("push-pull-panel");
    harness.state_mut().run(simple3d_core::keymap::Command::ModePushPull);
    harness.step();
    let plate = harness.state().scene.node(harness.state().scene.root()).children[0];
    let at = top_of_plate(&harness);
    drag(&mut harness, at, at - egui::vec2(0.0, 60.0), 6);
    harness.state_mut().evaluated = Evaluator::new().evaluate(&harness.state().scene, &Cancel::new());
    harness.step();
    let at = top_of_plate(&harness);
    drag(&mut harness, at, at + egui::vec2(0.0, 20.0), 6);
    harness.state_mut().evaluated = Evaluator::new().evaluate(&harness.state().scene, &Cancel::new());
    harness.step();
    let edited = harness.state().evaluated.mesh.signed_volume();

    // Each edit has its own row and buttons; the pull's are the second pair.
    harness.get_all_by_label("Remove").nth(1).expect("no row for the pull").click();
    harness.step();
    assert_eq!(harness.state().scene.node(plate).edits.len(), 1);
    harness.state_mut().evaluated = Evaluator::new().evaluate(&harness.state().scene, &Cancel::new());
    harness.step();
    let pushed = harness.state().evaluated.mesh.signed_volume();
    assert!(pushed > edited, "removing the pull did not give its material back");

    harness.get_by_label("Extract").click();
    harness.step();
    let app = harness.state();
    assert!(app.scene.node(plate).edits.is_empty());
    let extracted = *app.selection.last().unwrap();
    assert!(app.scene.node(extracted).is_extrusion());
    assert_eq!(app.scene.node(app.scene.root()).children, vec![plate, extracted]);
    let after = Evaluator::new().evaluate(&app.scene, &Cancel::new()).mesh.signed_volume();
    assert!((after - pushed).abs() < 1e-6, "extracting changed the model: {after} vs {pushed}");
}
