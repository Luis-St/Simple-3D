//! The align and distribute tool's window and its clicks (issue 70).

use super::*;
use crate::arrange_tool::{PathSource, Side};
use egui_kittest::kittest::Queryable;
use simple3d_core::keymap::Command;
use simple3d_core::scene::NodeId;
use simple3d_geom::Vec3;

/// A harness with boxes at `at`, selected in that order, and the tool open on them.
fn with_boxes(name: &str, at: &[Vec3]) -> (Harness<'static, App>, Vec<NodeId>) {
    let mut ids = Vec::new();
    let mut harness = harness_configured(name, |app| {
        let root = app.scene.root();
        for (index, &position) in at.iter().enumerate() {
            let id = app.scene.add_primitive("box", root, index + 1).expect("the box is in the registry");
            app.scene.get_mut(id).unwrap().position = position;
            ids.push(id);
        }
        app.selection = ids.clone();
        app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
    });
    harness.state_mut().run(Command::AlignDistribute);
    // A few frames, since the window settles over several.
    for _ in 0..6 {
        harness.step();
    }
    assert!(harness.state().arrange_tool.is_some(), "the tool did not open");
    (harness, ids)
}

#[test]
pub(crate) fn aligning_through_the_window_moves_the_boxes_in_one_undo_step() {
    let (mut harness, ids) = with_boxes("arrange-align", &[Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 15.0, 0.0)]);
    harness.state_mut().arrange_tool.as_mut().unwrap().align[1] = Some(Side::Min);
    harness.step();
    // Nothing has moved yet; the second box is drawn where it would go.
    assert_eq!(harness.state().scene.node(ids[1]).position.y, 15.0, "the box moved before Apply");
    let templates = crate::arrange_tool::templates(harness.state());
    assert_eq!(templates.len(), 1, "not one template for the one box that moves: {templates:?}");

    harness.get_by_label("Apply").click();
    harness.step();
    harness.step();
    let scene = &harness.state().scene;
    assert_eq!(scene.node(ids[1]).position.y, 0.0, "the box was not lined up on the other's low side");
    assert_eq!(scene.node(ids[1]).position.x, 40.0, "the box moved along an axis left alone");
    assert!(harness.state().arrange_tool.is_none(), "the window stayed open after Apply");
    assert_eq!(harness.state().history.undo_len(), 1, "the alignment is not one undo step");
}

#[test]
pub(crate) fn one_box_spread_along_a_drawn_path_brings_copies_beside_it() {
    let (mut harness, ids) = with_boxes("arrange-copies", &[Vec3::new(0.0, 0.0, 0.0)]);
    assert_eq!(harness.state().arrange_tool.as_ref().unwrap().mode, crate::arrange_tool::Arrange::Path);
    harness.get_by_label("Draw").click();
    harness.step();
    {
        let tool = harness.state_mut().arrange_tool.as_mut().unwrap();
        assert_eq!(tool.source, PathSource::Drawn, "the Draw chip did not choose drawing");
        tool.points = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(90.0, 0.0, 0.0)];
        tool.count = 4;
    }
    // The point rows settle over a few frames, moving the buttons below them.
    for _ in 0..4 {
        harness.step();
    }
    // The box already sits on the path's start, so only the three copies to come are drawn as templates.
    assert_eq!(crate::arrange_tool::templates(harness.state()).len(), 3);

    let root = harness.state().scene.root();
    let before = harness.state().scene.node(root).children.len();
    harness.get_by_label("Apply").click();
    harness.step();
    harness.step();
    let scene = &harness.state().scene;
    let children = &scene.node(root).children;
    assert_eq!(children.len(), before + 3, "three copies were not made in the box's group");
    // Right after the original, in path order, 30 mm apart.
    let at = children.iter().position(|&c| c == ids[0]).unwrap();
    let xs: Vec<f64> = children[at..at + 4].iter().map(|&c| scene.node(c).position.x).collect();
    let first = xs[0];
    for (i, x) in xs.iter().enumerate() {
        assert!((x - first - 30.0 * i as f64).abs() < 1e-6, "the copies are not evenly spaced: {xs:?}");
    }
    assert_eq!(harness.state().selection.len(), 4, "the spread objects are not left selected");

    harness.state_mut().run(Command::Undo);
    harness.step();
    assert_eq!(harness.state().scene.node(root).children.len(), before, "one undo did not take the copies back");
}

#[test]
pub(crate) fn clicks_in_the_viewport_draw_the_path_and_a_right_click_takes_one_back() {
    let (mut harness, _) = with_boxes("arrange-draw", &[Vec3::new(0.0, 0.0, 0.0)]);
    harness.state_mut().arrange_tool.as_mut().unwrap().source = PathSource::Drawn;
    harness.step();
    let viewport = harness.state().viewport_rect;
    for at in [viewport.center() - egui::vec2(150.0, -120.0), viewport.center() + egui::vec2(-60.0, 150.0)] {
        press(&mut harness, at);
        release(&mut harness, at);
    }
    assert_eq!(harness.state().arrange_tool.as_ref().unwrap().points.len(), 2, "two clicks did not add two points");
    // The selection is the tool's business now, so clicking did not change it.
    assert_eq!(harness.state().selection.len(), 1);
    let at = viewport.center();
    button(&mut harness, at, egui::PointerButton::Secondary, true);
    button(&mut harness, at, egui::PointerButton::Secondary, false);
    harness.step();
    assert_eq!(harness.state().arrange_tool.as_ref().unwrap().points.len(), 1, "the right-click took nothing back");
}

#[test]
pub(crate) fn clicking_a_box_edge_picks_it_for_the_path_and_clicking_it_again_drops_it() {
    let (mut harness, _) = with_boxes("arrange-edges", &[Vec3::new(0.0, 0.0, 0.0)]);
    let view = harness.state().current_view();
    // An edge the picture shows, found the way the pointer would.
    let edges = harness.state().model_edges();
    let target = edges
        .iter()
        .filter_map(|&(a, b)| view.project((a + b) * 0.5).map(|(at, _)| at))
        .find(|&at| harness.state().nearest_model_edge(&view, at).is_some())
        .expect("no edge of the box is in sight");
    press(&mut harness, target);
    release(&mut harness, target);
    let runs = harness.state().arrange_tool.as_ref().unwrap().runs.clone();
    assert_eq!(runs.len(), 1, "the click picked no edge");
    // A box edge meets the next at a right angle, so it comes alone even taking whole runs.
    assert_eq!(runs[0].len(), 1, "a box edge came with others: {runs:?}");
    press(&mut harness, target);
    release(&mut harness, target);
    assert!(harness.state().arrange_tool.as_ref().unwrap().runs.is_empty(), "clicking the edge again kept it");
}

/// Regression: picking read each shape's own edges, so on a dodecahedron with its top half cut away
/// by a box the cutter's axis-parallel edges won along the cut, and edges of the removed half could be
/// picked in thin air (of 14 result edges in sight, 11 caught something, rarely the edge pointed at).
#[test]
pub(crate) fn only_the_edges_of_a_cut_shape_that_are_drawn_can_be_picked() {
    let mut harness = harness_configured("arrange-cut", |app| {
        let root = app.scene.root();
        let plate = app.scene.node(root).children[0];
        app.scene.remove(plate);
        let group = app.scene.add_group(simple3d_core::scene::GroupOp::Difference, root, 0);
        app.scene.add_primitive("dodecahedron", group, 0).unwrap();
        let cutter = app.scene.add_primitive("box", group, 1).unwrap();
        let node = app.scene.get_mut(cutter).unwrap();
        node.position = Vec3::new(0.0, 0.0, 25.0);
        node.scale = Vec3::new(3.0, 3.0, 2.5);
        app.selection = vec![group];
        app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
        app.frame_all();
    });
    harness.state_mut().run(Command::AlignDistribute);
    harness.step();
    let app = harness.state();
    let view = app.current_view();
    let edges = app.model_edges();
    // Every edge of the cut outline, which runs round the dodecahedron at a slant to the axes.
    let slanted = |&(a, b): &(Vec3, Vec3)| {
        let d = (b - a).normalized();
        [d.x, d.y, d.z].iter().all(|c| c.abs() < 0.999)
    };
    let mut picked = 0;
    for &(a, b) in edges.iter() {
        let middle = (a + b) * 0.5;
        let Some((at, _)) = view.project(middle) else { continue };
        if app.nearest_model_edge(&view, at).is_none() {
            continue;
        }
        let edge = app.nearest_model_edge(&view, at).unwrap();
        // Whatever is caught is part of what is left: nothing above the cut.
        assert!(edge.0.z < 1e-6 && edge.1.z < 1e-6, "an edge of the cut-away half was picked: {edge:?}");
        picked += slanted(&edge) as usize;
    }
    assert!(picked >= 5, "only {picked} slanted edges of the cut dodecahedron could be picked");
}

/// Regression: the count rounded each frame's movement away, so a slow drag needed a long way per step.
#[test]
pub(crate) fn a_slow_drag_on_the_count_steps_it_like_every_other_number() {
    let (mut harness, _) = with_boxes("arrange-count", &[Vec3::new(0.0, 0.0, 0.0)]);
    let count = |harness: &Harness<'_, App>| harness.state().arrange_tool.as_ref().expect("the tool is open").count;
    assert_eq!(count(&harness), 5);
    // 60 px at 6 px per step is ten more, in 30 frames of 2 px, each a third of a step; before the
    // fix it added nothing. The first few pixels go to egui's drag threshold, so one step may be short.
    let field = rect_of(&harness, crate::panel_properties::grip_id("arrange-count"));
    drag(&mut harness, field.center(), field.center() + egui::vec2(60.0, 0.0), 30);
    assert!((14..=15).contains(&count(&harness)), "a slow 60 px drag added {} rather than ten", count(&harness) - 5);
}

/// Regression: the group row selected alone was one object, so the tool opened along a path and
/// aligning did nothing. Lining up now works on what the group holds.
#[test]
pub(crate) fn a_group_selected_alone_lines_up_its_contents() {
    let mut children = Vec::new();
    let mut harness = harness_configured("arrange-group", |app| {
        let root = app.scene.root();
        let group = app.scene.add_group(simple3d_core::scene::GroupOp::Assembly, root, 1);
        for (index, y) in [0.0, 25.0, -40.0].into_iter().enumerate() {
            let id = app.scene.add_primitive("box", group, index).unwrap();
            app.scene.get_mut(id).unwrap().position = Vec3::new(index as f64 * 40.0, y, 0.0);
            children.push(id);
        }
        app.select_only(group);
        app.evaluated = Evaluator::new().evaluate(&app.scene, &Cancel::new());
    });
    harness.state_mut().run(Command::AlignDistribute);
    for _ in 0..6 {
        harness.step();
    }
    assert_eq!(harness.state().arrange_tool.as_ref().unwrap().mode, crate::arrange_tool::Arrange::Align);
    assert!(harness.query_by_label("Arranging the 3 objects in Group.").is_some(), "the window does not say so");
    harness.state_mut().arrange_tool.as_mut().unwrap().align[1] = Some(Side::Min);
    harness.step();
    harness.get_by_label("Apply").click();
    harness.step();
    harness.step();
    let ys: Vec<f64> = children.iter().map(|&id| harness.state().scene.node(id).position.y).collect();
    assert_eq!(ys, vec![-40.0, -40.0, -40.0], "the group's contents were not lined up");
}
